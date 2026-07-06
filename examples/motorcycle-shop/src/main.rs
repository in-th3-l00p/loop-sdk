use std::collections::BTreeMap;
use std::sync::Mutex;

use lib::prelude::*;

#[derive(Schema, Clone)]
struct Motorcycle {
    #[check(min_len = 1)]
    brand: String,
    #[check(min_len = 1)]
    model: String,
    #[check(min = 1885, max = 2100)]
    year: u32,
    #[check(min = 0.0)]
    price: f64,
    #[check(min_len = 1)]
    nickname: Option<String>,
}

#[derive(Schema)]
struct Listing {
    id: u64,
    motorcycle: Motorcycle,
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
fn create(motorcycle: Motorcycle) -> u64 {
    let mut shop = SHOP.lock().unwrap();
    shop.next_id += 1;
    let id = shop.next_id;
    shop.inventory.insert(id, motorcycle);
    id
}

#[rest(get, "/motorcycles")]
fn list() -> Vec<Listing> {
    let shop = SHOP.lock().unwrap();
    shop.inventory
        .iter()
        .map(|(id, motorcycle)| Listing {
            id: *id,
            motorcycle: motorcycle.clone(),
        })
        .collect()
}

#[rest(get, "/motorcycles/{id}")]
fn get(id: u64) -> Result<Listing, HandlerError> {
    let shop = SHOP.lock().unwrap();
    let motorcycle = shop.inventory.get(&id).ok_or(not_found(id))?;
    Ok(Listing {
        id,
        motorcycle: motorcycle.clone(),
    })
}

#[rest(put, "/motorcycles/{id}")]
fn update(id: u64, motorcycle: Motorcycle) -> Result<bool, HandlerError> {
    let mut shop = SHOP.lock().unwrap();
    let existing = shop.inventory.get_mut(&id).ok_or(not_found(id))?;
    *existing = motorcycle;
    Ok(true)
}

#[rest(delete, "/motorcycles/{id}")]
fn delete(id: u64) -> Result<bool, HandlerError> {
    let mut shop = SHOP.lock().unwrap();
    shop.inventory.remove(&id).ok_or(not_found(id))?;
    Ok(true)
}

fn not_found(id: u64) -> HandlerError {
    format!("no motorcycle with id {id}").into()
}

fn main() {
    lib::server::run();
}
