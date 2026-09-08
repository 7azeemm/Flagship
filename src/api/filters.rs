use crate::core::product::Product;
use crate::core::sections::Section;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;

pub static STRUCTURED_FILTERS: &[&str] = &["cpu", "gpu", "model"];

#[derive(Debug, Serialize)]
pub struct FilterGroup {
    pub key: String,
    pub label: String,
    pub options: Vec<FilterValue>,
}

#[derive(Debug, Serialize)]
pub struct FilterValue {
    pub id: String,
    pub label: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FilterValue>,
}

fn extract_numeric_value(label: &str) -> Option<f64> {
    let num_str: String = label
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if num_str.is_empty() || num_str == "." {
        return None;
    }
    let mut number: f64 = num_str.parse().ok()?;
    if label.contains("TB") {
        number *= 1024.0; // convert TB to GB-equivalent
    }
    Some(number)
}

fn sort_options(key: &str, options: &mut [FilterValue]) {
    static FIXED_SETS: &[(&str, &[&str])] = &[
        ("gpu", &["RTX", "GTX", "Radeon", "Arc", "Intel", "AMD"]),
        ("cpu", &["Intel", "AMD"]),
    ];

    let current_set = FIXED_SETS.iter().find(|(k, _)| k == &key).map(|(_, s)| *s);

    options.sort_by(|a, b| {
        // 1) "Others" always last
        let others_cmp = a.id.ends_with("Others").cmp(&b.id.ends_with("Others"));
        if others_cmp != Ordering::Equal {
            return others_cmp;
        }

        // 2) Fixed ordering
        if let Some(set) = current_set {
            let a_pos = set.iter().position(|p| a.label.eq_ignore_ascii_case(p));
            let b_pos = set.iter().position(|p| b.label.eq_ignore_ascii_case(p));

            match (a_pos, b_pos) {
                (Some(pa), Some(pb)) => return pa.cmp(&pb),
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => {}
            }
        }

        // 3) Numeric ordering
        if let (Some(an), Some(bn)) = (extract_numeric_value(&a.label), extract_numeric_value(&b.label)) {
            return an.total_cmp(&bn);
        }

        // 4) Count/Alphabetic ordering
        if key == "model" {
            b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label))
        } else {
            a.label.cmp(&b.label).then_with(|| b.count.cmp(&a.count))
        }
    });

    for opt in options.iter_mut() {
        sort_options(key, &mut opt.children);
    }
}

fn collapse_single_child_groups(options: &mut [FilterValue]) {
    for node in options.iter_mut() {
        collapse_children(&mut node.children);
    }

    // For top keys only
    for node in options.iter_mut() {
        if node.count == 0 && node.children.len() == 1 {
            if node.label == node.children[0].label || node.id == "Others" && node.children[0].id == "Others/Others" {
                *node = node.children.pop().unwrap();
            }
        }
    }
}

fn collapse_children(children: &mut Vec<FilterValue>) {
    for child in children.iter_mut() {
        collapse_children(&mut child.children);
    }
    for child in children.iter_mut() {
        if child.count == 0 && child.children.len() == 1 && child.label.contains(' ') {
            *child = child.children.pop().unwrap();
        }
    }
}

fn insert_count(
    nodes: &mut Vec<FilterValue>,
    prefix: &str,
    segments: &[&str],
    leaf_label: &str,
    count: usize,
) {
    let seg = segments[0];
    let id = if prefix.is_empty() { seg.to_string() } else { format!("{prefix}/{seg}") };

    let idx = match nodes.iter().position(|n| n.id == id) {
        Some(i) => i,
        None => {
            nodes.push(FilterValue {
                id: id.clone(),
                label: seg.to_string(),
                count: 0,
                children: vec![],
            });
            nodes.len() - 1
        }
    };

    if segments.len() == 1 {
        nodes[idx].count += count; // direct match on this exact id
        if !leaf_label.is_empty() {
            nodes[idx].label = leaf_label.to_string();
        }
    } else {
        insert_count(
            &mut nodes[idx].children,
            &id,
            &segments[1..],
            leaf_label,
            count,
        );
    }
}

/// Rolls child counts up into every ancestor.
fn finalize_counts(options: &mut [FilterValue]) {
    for node in options.iter_mut() {
        finalize_counts(&mut node.children);
        let children_total: usize = node.children.iter().map(|c| c.count).sum();
        node.count += children_total;
    }
}

fn build_filter_group(key: &str, label: &str, products: &[&Product]) -> FilterGroup {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for p in products {
        if let Some(v) = p.filter_ids.get(key) {
            *counts.entry(v.clone()).or_insert(0) += 1;
        }
    }

    let mut options: Vec<FilterValue> = Vec::new();
    for (id, count) in &counts {
        let segments: Vec<&str> = id.split('/').collect();
        let seg_label = segments.last().copied().unwrap_or(id.as_str()).to_string();
        insert_count(&mut options, "", &segments, &seg_label, *count);
    }

    let missing = products
        .iter()
        .filter(|p| !p.filter_ids.contains_key(key))
        .count();
    if missing > 0 {
        insert_count(&mut options, "", &["Others", "Others"], "Others", missing);
    }

    collapse_single_child_groups(&mut options);
    finalize_counts(&mut options);
    sort_options(key, &mut options);

    FilterGroup {
        key: key.to_string(),
        label: label.to_string(),
        options,
    }
}

fn label_from_key(key: &str, structured: bool) -> String {
    if structured {
        return key.to_uppercase();
    }
    key.split('_')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Products that satisfy every active selection EXCEPT exclude_key's own.
fn scoped_products<'a>(
    products: &'a [Product],
    selections: &HashMap<String, Vec<String>>,
    exclude_key: &str,
) -> Vec<&'a Product> {
    products
        .iter()
        .filter(|p| {
            selections.iter().all(|(key, ids)| {
                key == exclude_key || ids.is_empty() || product_matches_key(p, key, ids)
            })
        })
        .collect()
}

pub fn product_matches_key(product: &Product, key: &str, selected_ids: &[String]) -> bool {
    let value = product
        .filter_ids
        .get(key)
        .map(String::as_str)
        .unwrap_or("Others/Others");
    // The most specific rule wins. !path excludes a child of a selected parent.
    selected_ids.iter()
        .filter(|rule| {
            let path = rule.strip_prefix('!').unwrap_or(rule);
            value == path || value.strip_prefix(path).is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|rule| rule.trim_start_matches('!').len())
        .is_some_and(|rule| !rule.starts_with('!'))
}

pub fn build_all_filters(
    section: Section,
    products: &[Product],
    selections: &HashMap<String, Vec<String>>,
) -> Vec<FilterGroup> {
    let keys = &section.config().filters;
    let mut groups = Vec::with_capacity(keys.len());

    for key in keys {
        let structured = STRUCTURED_FILTERS.contains(&key.as_str());
        let scoped = scoped_products(products, selections, key);
        let group = build_filter_group(key, &label_from_key(key, structured), &scoped);
        groups.push(group);
    }
    groups
}
