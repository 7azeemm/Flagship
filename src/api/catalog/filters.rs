use crate::core::product::{Product, is_structured_component};
use crate::core::sections::Section;
use serde::Serialize;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct FilterGroup {
    pub key: String,
    pub label: String,
    pub options: Vec<FilterValue>,
}

#[derive(Debug, Serialize)]
pub struct FilterValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub label: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FilterValue>,
}

fn extract_numeric_value(label: &str) -> Option<f64> {
    let num_str: String = label.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    if num_str.is_empty() || num_str == "." {
        return None;
    }
    let mut number: f64 = num_str.parse().ok()?;
    if label.contains("TB") {
        number *= 1024.0;
    }
    Some(number)
}

fn sort_options(key: &str, options: &mut [FilterValue]) {
    const FIXED_SETS: &[(&str, &[&str])] = &[("gpu", &["RTX", "GTX", "Radeon", "Arc", "Intel", "AMD"]), ("cpu", &["Intel", "AMD"])];

    let current_set = FIXED_SETS.iter().find(|(k, _)| k == &key).map(|(_, s)| *s);
    options.sort_by(|a, b| {
        let a_value = a.id.as_deref().unwrap_or(&a.label);
        let b_value = b.id.as_deref().unwrap_or(&b.label);

        // 1) "Others" always last
        let others_cmp = a_value.ends_with("Others").cmp(&b_value.ends_with("Others"));
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

    for option in options {
        sort_options(key, &mut option.children);
    }
}

fn collapse_single_child_groups(options: &mut [FilterValue]) {
    for node in options.iter_mut() {
        collapse_children(&mut node.children);
    }

    // For top keys only
    for node in options.iter_mut() {
        if node.count == 0 && node.children.len() == 1 {
            let child = &node.children[0];
            let repeated_label = node.label == child.label;
            let repeated_others = node.id.as_deref() == Some("Others") && child.id.as_deref() == Some("Others/Others");
            if repeated_label || repeated_others {
                *node = node.children.pop().unwrap();
            }
        }
    }
}

fn collapse_children(children: &mut [FilterValue]) {
    for child in children.iter_mut() {
        collapse_children(&mut child.children);
    }
    for child in children.iter_mut() {
        if child.count == 0 && child.children.len() == 1 && child.label.contains(' ') {
            *child = child.children.pop().unwrap();
        }
    }
}

fn insert_count(nodes: &mut Vec<FilterValue>, prefix: &str, segments: &[&str], leaf_label: &str, count: usize, structured: bool) {
    let segment = segments[0];
    let path = if prefix.is_empty() { segment.to_string() } else { format!("{prefix}/{segment}") };

    let index = nodes
        .iter()
        .position(|node| if structured { node.id.as_deref() == Some(path.as_str()) } else { node.label == segment })
        .unwrap_or_else(|| {
            nodes.push(FilterValue {
                id: structured.then_some(path.clone()),
                label: segment.to_string(),
                count: 0,
                children: Vec::new(),
            });
            nodes.len() - 1
        });

    if segments.len() == 1 {
        nodes[index].count += count;
        if !leaf_label.is_empty() {
            nodes[index].label = leaf_label.to_string();
        }
    } else {
        insert_count(&mut nodes[index].children, &path, &segments[1..], leaf_label, count, structured);
    }
}

fn finalize_counts(options: &mut [FilterValue]) {
    for node in options {
        finalize_counts(&mut node.children);
        node.count += node.children.iter().map(|child| child.count).sum::<usize>();
    }
}

fn build_filter_group(key: &str, label: &str, products: &[&Product]) -> FilterGroup {
    let structured = is_structured_component(key);
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();

    for product in products {
        let Some(component) = product.components.get(key) else {
            continue;
        };
        let component_label = component.label().to_string();
        let value = catalog_filter_value(product, key).into_owned();
        let entry = counts.entry(value).or_insert((component_label.clone(), 0));
        entry.1 += 1;
    }

    let mut options = Vec::new();
    for (value, (value_label, count)) in counts {
        let segments: Vec<&str> = value.split('/').collect();
        insert_count(&mut options, "", &segments, &value_label, count, structured);
    }

    let missing = products.iter().filter(|product| !product.components.contains_key(key)).count();
    if missing > 0 {
        let path = if structured { vec!["Others", "Others"] } else { vec!["Others"] };
        insert_count(&mut options, "", &path, "Others", missing, structured);
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
        .filter(|part| !part.is_empty())
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

fn scoped_products<'a>(products: &[&'a Product], selections: &HashMap<String, Vec<String>>, exclude_key: &str) -> Vec<&'a Product> {
    products
        .iter()
        .copied()
        .filter(|product| {
            selections
                .iter()
                .all(|(key, values)| key == exclude_key || values.is_empty() || product_matches_key(product, key, values))
        })
        .collect()
}

/// Use the same value for facet construction, matching and statistics links.
pub(crate) fn catalog_filter_value<'a>(product: &'a Product, key: &str) -> Cow<'a, str> {
    let structured = is_structured_component(key);
    let component = product.components.get(key);
    if structured {
        match component {
            Some(component) => component.id().map(Cow::Borrowed).unwrap_or_else(|| Cow::Owned(format!("Others/{}", component.label()))),
            None => Cow::Borrowed("Others/Others"),
        }
    } else {
        Cow::Borrowed(component.map(|value| value.label()).unwrap_or("Others"))
    }
}

pub fn product_matches_key(product: &Product, key: &str, selected_values: &[String]) -> bool {
    let structured = is_structured_component(key);
    let value = catalog_filter_value(product, key);

    if !structured {
        return selected_values.iter().any(|selected| selected == value.as_ref());
    }

    selected_values
        .iter()
        .filter(|rule| {
            let path = rule.strip_prefix('!').unwrap_or(rule);
            value == path || value.strip_prefix(path).is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|rule| rule.trim_start_matches('!').len())
        .is_some_and(|rule| !rule.starts_with('!'))
}

pub fn build_all_filters(section: Section, products: &[&Product], selections: &HashMap<String, Vec<String>>) -> Vec<FilterGroup> {
    let keys = &section.config().filters;
    let mut groups = Vec::with_capacity(keys.len());

    for key in keys {
        let structured = is_structured_component(key);
        let scoped = scoped_products(products, selections, key);
        groups.push(build_filter_group(key, &label_from_key(key, structured), &scoped));
    }
    groups
}
