use crate::api::error::ApiResult;
use crate::core::sections::Section;
use axum::Json;
use indexmap::IndexMap;
use serde::Serialize;

#[derive(Serialize)]
pub struct ServerInfo {
    pub categories: IndexMap<&'static str, Vec<Category>>,
}

#[derive(Serialize)]
pub struct Category {
    pub id: Section,
    pub name: &'static str,
}

pub async fn get_info() -> ApiResult<Json<ServerInfo>> {
    Ok(Json(server_info()))
}

fn server_info() -> ServerInfo {
    use Section::*;
    let groups: [(&str, Vec<(Section, &str)>); 5] = [
        (
            "Computers",
            vec![
                (PC, "Desktop PCs"),
                (GamingLaptop, "Gaming Laptops"),
                (Laptop, "Laptops"),
                (GamingPC, "Gaming PCs"),
                (MacBook, "MacBooks"),
                (AllInOnePC, "All-In-One PCs"),
                (MiniPC, "Mini PCs"),
            ],
        ),
        (
            "Components",
            vec![
                (CPU, "Processors"),
                (GPU, "Graphics Cards"),
                (Memory, "Memory"),
                (Storage, "Storage"),
                (Motherboard, "Motherboards"),
                (PowerSupply, "Power Supplies"),
                (Cooler, "CPU Coolers"),
                (Case, "Cases"),
                (UpgradeKit, "Upgrade kits"),
            ],
        ),
        (
            "Peripherals",
            vec![
                (Monitor, "Monitors"),
                (Mouse, "Mouses"),
                (Keyboard, "Keyboards"),
                (Headphones, "Headphones"),
                (MousePad, "Mouse Pads"),
                (GamingChair, "Gaming Chairs"),
                (AccessoriesCombo, "Accessory Bundles"),
            ],
        ),
        (
            "Consoles",
            vec![
                (Console, "Consoles"),
                (Controller, "Controllers"),
                (ConsoleGame, "Console Games"),
                (ConsoleAccessories, "Console Accessories")
            ],
        ),
        (
            "Smartphones & TVs",
            vec![
                (Smartphone, "Smartphones"),
                (Tablet, "Tablets"),
                (Smartwatch, "Smartwatches"),
                (Television, "Televisions"),
                (Others, "Other products"),
            ],
        ),
    ];
    ServerInfo {
        categories: groups.into_iter().map(|(group, items)| 
            (group, items.into_iter().map(|(id, name)| Category { id, name }).collect())
        ).collect(),
    }
}