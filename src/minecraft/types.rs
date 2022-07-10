use super::proto::GameMode;
use mcproto_rs::types::{Slot};
use mcproto_rs::uuid::UUID4;
use std::convert::TryInto;

pub struct Health {
    pub health: i32,
    pub hunger: i32,
    pub saturation: i32,
}

pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub dimension: i32,
    pub world: String,
}

pub struct Player {
    pub name: String,
    pub uuid: UUID4,
    pub entity_id: i32,
    pub position: Position,
    pub health: Health,
    pub inventory: PlayerInventory,
    pub gamemode: GameMode,
}

impl Player {
    pub fn new(name: String, uuid: UUID4, entity_id: i32) -> Self {
        Player {
            name,
            uuid,
            entity_id,
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                dimension: 0,
                world: "World".to_string(),
            },
            health: Health {
                health: 20,
                hunger: 20,
                saturation: 20,
            },
            inventory: PlayerInventory::new_empty(),
            gamemode: GameMode::Spectator,
        }
    }
}

pub struct PlayerInventory {
    pub items: [Slot; 44],
}

impl PlayerInventory {
    pub fn new(items: [Slot; 44]) -> Self {
        Self { items }
    }

    pub fn new_empty() -> Self {
        // Stupid way to initialize an array of None without making Slot implement Copy
        let nothing: [Option<()>; 44] = [None; 44];
        let items: [Slot; 44] = nothing.map(|_| None);
        Self { items }
    }

    pub fn get_armor(&self) -> [Slot; 4] {
        self.items.clone()[5..8]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_inventory(&self) -> [Slot; 27] {
        self.items.clone()[9..35]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_hotbar(&self) -> [Slot; 9] {
        self.items.clone()[36..44]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_offhand(&self) -> Slot {
        self.items[45].clone()
    }

    pub fn get_crafting_input(&self) -> [Slot; 4] {
        self.items.clone()[1..4]
            .to_owned()
            .try_into()
            .expect("Inventory did not have enough slots.")
    }

    pub fn get_crafting_output(&self) -> Slot {
        self.items[0].clone()
    }
}
