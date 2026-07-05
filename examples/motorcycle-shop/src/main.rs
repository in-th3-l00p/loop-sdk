/* a complete REST CRUD API for a motorcycle shop, built on the loop SDK
with native (in-process) endpoint handlers sharing one in-memory store */

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use http::Method;
use lib::endpoint::engine::Engine;
use lib::endpoint::{Access, Binding, Endpoint, HandlerError, Parameter, Signature};
use lib::schema::{Primitive, Schema, Value};

#[derive(Clone)]
struct Motorcycle {
    brand: String,
    model: String,
    year: u32,
    price: f64,
}

#[derive(Default)]
struct Shop {
    inventory: BTreeMap<u64, Motorcycle>,
    next_id: u64,
}

type SharedShop = Arc<Mutex<Shop>>;

fn main() {
    let shop = SharedShop::default();

    let endpoints = vec![
        create_endpoint(shop.clone()),
        list_endpoint(shop.clone()),
        get_endpoint(shop.clone()),
        update_endpoint(shop.clone()),
        delete_endpoint(shop),
    ];

    let engine = Engine::new(endpoints).expect("invalid endpoint definitions");
    println!("motorcycle shop listening on http://127.0.0.1:3000");
    for route in lib::server::routes(&engine) {
        println!("  {route}");
    }
    lib::server::serve_blocking(engine, ("127.0.0.1", 3000)).expect("server failed");
}

// POST /motorcycles {brand, model, year, price} -> id
fn create_endpoint(shop: SharedShop) -> Endpoint {
    Endpoint {
        name: "create".into(),
        signature: Signature {
            params: motorcycle_params(),
            output: Schema::Primitive(Primitive::U64),
        },
        access: Access::Rest {
            method: Method::POST,
            url: "/motorcycles".into(),
        },
        binding: Binding::Native(Arc::new(move |args: &[Value]| {
            let motorcycle = motorcycle_from_args(args)?;
            let mut shop = shop.lock().unwrap();
            shop.next_id += 1;
            let id = shop.next_id;
            shop.inventory.insert(id, motorcycle);
            Ok(Value::U64(id))
        })),
    }
}

// GET /motorcycles -> [{id, brand, model, year, price}]
fn list_endpoint(shop: SharedShop) -> Endpoint {
    Endpoint {
        name: "list".into(),
        signature: Signature {
            params: vec![],
            output: Schema::List(Box::new(record_schema())),
        },
        access: Access::Rest {
            method: Method::GET,
            url: "/motorcycles".into(),
        },
        binding: Binding::Native(Arc::new(move |_: &[Value]| {
            let shop = shop.lock().unwrap();
            let records = shop
                .inventory
                .iter()
                .map(|(id, motorcycle)| record(*id, motorcycle))
                .collect();
            Ok(Value::List(records))
        })),
    }
}

// GET /motorcycles/{id} -> {id, brand, model, year, price}
fn get_endpoint(shop: SharedShop) -> Endpoint {
    Endpoint {
        name: "get".into(),
        signature: Signature {
            params: vec![param("id", Primitive::U64)],
            output: record_schema(),
        },
        access: Access::Rest {
            method: Method::GET,
            url: "/motorcycles/{id}".into(),
        },
        binding: Binding::Native(Arc::new(move |args: &[Value]| {
            let id = id_from_args(args)?;
            let shop = shop.lock().unwrap();
            let motorcycle = shop.inventory.get(&id).ok_or(not_found(id))?;
            Ok(record(id, motorcycle))
        })),
    }
}

// PUT /motorcycles/{id} {brand, model, year, price} -> true
fn update_endpoint(shop: SharedShop) -> Endpoint {
    let mut params = vec![param("id", Primitive::U64)];
    params.extend(motorcycle_params());

    Endpoint {
        name: "update".into(),
        signature: Signature {
            params,
            output: Schema::Primitive(Primitive::Bool),
        },
        access: Access::Rest {
            method: Method::PUT,
            url: "/motorcycles/{id}".into(),
        },
        binding: Binding::Native(Arc::new(move |args: &[Value]| {
            let id = id_from_args(args)?;
            let motorcycle = motorcycle_from_args(&args[1..])?;
            let mut shop = shop.lock().unwrap();
            match shop.inventory.get_mut(&id) {
                Some(existing) => {
                    *existing = motorcycle;
                    Ok(Value::Bool(true))
                }
                None => Err(not_found(id)),
            }
        })),
    }
}

// DELETE /motorcycles/{id} -> true
fn delete_endpoint(shop: SharedShop) -> Endpoint {
    Endpoint {
        name: "delete".into(),
        signature: Signature {
            params: vec![param("id", Primitive::U64)],
            output: Schema::Primitive(Primitive::Bool),
        },
        access: Access::Rest {
            method: Method::DELETE,
            url: "/motorcycles/{id}".into(),
        },
        binding: Binding::Native(Arc::new(move |args: &[Value]| {
            let id = id_from_args(args)?;
            let mut shop = shop.lock().unwrap();
            match shop.inventory.remove(&id) {
                Some(_) => Ok(Value::Bool(true)),
                None => Err(not_found(id)),
            }
        })),
    }
}

fn motorcycle_params() -> Vec<Parameter> {
    vec![
        param("brand", Primitive::Str),
        param("model", Primitive::Str),
        param("year", Primitive::U32),
        param("price", Primitive::F64),
    ]
}

fn param(name: &str, primitive: Primitive) -> Parameter {
    Parameter {
        name: name.into(),
        schema: Schema::Primitive(primitive),
    }
}

// records go out as string-to-string maps until the schema grows a proper
// record type with per-field schemas
fn record_schema() -> Schema {
    Schema::Map(
        Box::new(Schema::Primitive(Primitive::Str)),
        Box::new(Schema::Primitive(Primitive::Str)),
    )
}

fn record(id: u64, motorcycle: &Motorcycle) -> Value {
    let field = |key: &str, value: String| (Value::Str(key.into()), Value::Str(value));
    Value::Map(vec![
        field("id", id.to_string()),
        field("brand", motorcycle.brand.clone()),
        field("model", motorcycle.model.clone()),
        field("year", motorcycle.year.to_string()),
        field("price", motorcycle.price.to_string()),
    ])
}

fn motorcycle_from_args(args: &[Value]) -> Result<Motorcycle, HandlerError> {
    let [
        Value::Str(brand),
        Value::Str(model),
        Value::U32(year),
        Value::F64(price),
    ] = args
    else {
        return Err("expected brand, model, year, price".into());
    };
    Ok(Motorcycle {
        brand: brand.clone(),
        model: model.clone(),
        year: *year,
        price: *price,
    })
}

fn id_from_args(args: &[Value]) -> Result<u64, HandlerError> {
    match args.first() {
        Some(Value::U64(id)) => Ok(*id),
        _ => Err("expected an id".into()),
    }
}

fn not_found(id: u64) -> HandlerError {
    format!("no motorcycle with id {id}").into()
}
