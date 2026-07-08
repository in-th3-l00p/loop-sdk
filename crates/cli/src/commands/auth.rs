pub fn secret_new() {
    let secret = lib::auth::generate_secret();
    println!("secret: {secret}");
    println!();
    println!("store it somewhere safe (an env var, a secret manager) and");
    println!("point loop.toml at it:");
    println!();
    println!("  [auth]");
    println!("  secret = \"env:LOOP_AUTH_SECRET\"");
}
