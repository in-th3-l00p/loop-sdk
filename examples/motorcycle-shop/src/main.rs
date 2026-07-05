/* a complete REST CRUD API for a motorcycle shop, built with the loop SDK
attribute UX: schemas come from the function signatures */

use std::collections::BTreeMap;
use std::sync::Mutex;

use lib::prelude::*;

#[derive(Clone)]
struct Motorcycle {
    brand: String,
    model: String,
    year: u32,
    price: f64,
}

struct Shop {
    inventory: BTreeMap<u64, Motorcycle>,
    next_id: u64,
}

static SHOP: Mutex<Shop> = Mutex::new(Shop {
    inventory: BTreeMap::new(),
    next_id: 0,
});

#[rest(post, "/motorcycles")]
fn create(brand: String, model: String, year: u32, price: f64) -> u64 {
    let mut shop = SHOP.lock().unwrap();
    shop.next_id += 1;
    let id = shop.next_id;
    shop.inventory.insert(
        id,
        Motorcycle {
            brand,
            model,
            year,
            price,
        },
    );
    id
}

#[rest(get, "/motorcycles")]
fn list() -> Vec<BTreeMap<String, String>> {
    let shop = SHOP.lock().unwrap();
    shop.inventory
        .iter()
        .map(|(id, motorcycle)| record(*id, motorcycle))
        .collect()
}

#[rest(get, "/motorcycles/{id}")]
fn get(id: u64) -> Result<BTreeMap<String, String>, HandlerError> {
    let shop = SHOP.lock().unwrap();
    let motorcycle = shop.inventory.get(&id).ok_or(not_found(id))?;
    Ok(record(id, motorcycle))
}

#[rest(put, "/motorcycles/{id}")]
fn update(
    id: u64,
    brand: String,
    model: String,
    year: u32,
    price: f64,
) -> Result<bool, HandlerError> {
    let mut shop = SHOP.lock().unwrap();
    let motorcycle = shop.inventory.get_mut(&id).ok_or(not_found(id))?;
    *motorcycle = Motorcycle {
        brand,
        model,
        year,
        price,
    };
    Ok(true)
}

#[rest(delete, "/motorcycles/{id}")]
fn delete(id: u64) -> Result<bool, HandlerError> {
    let mut shop = SHOP.lock().unwrap();
    shop.inventory.remove(&id).ok_or(not_found(id))?;
    Ok(true)
}

// records go out as string-to-string maps until the schema grows a proper
// record type with per-field schemas
fn record(id: u64, motorcycle: &Motorcycle) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("id".to_string(), id.to_string()),
        ("brand".to_string(), motorcycle.brand.clone()),
        ("model".to_string(), motorcycle.model.clone()),
        ("year".to_string(), motorcycle.year.to_string()),
        ("price".to_string(), motorcycle.price.to_string()),
    ])
}

fn not_found(id: u64) -> HandlerError {
    format!("no motorcycle with id {id}").into()
}

fn main() {
    lib::server::run();
}
