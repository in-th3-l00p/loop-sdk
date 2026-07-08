/* a stand-in for the browser wallet during development: signs a SIWE
message (EIP-191) with a local key so the /auth/wallet/verify flow can be
driven from the terminal. usage:

    KEY=0x… MESSAGE_FILE=message.txt cargo run --bin sign            */

use lib::eth::Treasury;

fn main() {
    let key = std::env::var("KEY").expect("set KEY to a 0x-hex private key");
    let path = std::env::var("MESSAGE_FILE").expect("set MESSAGE_FILE to the SIWE message file");
    let message = std::fs::read_to_string(path).expect("read message file");

    let signer = Treasury::from_key(&key).expect("valid key");
    let signature = signer
        .sign_message(message.trim_end_matches('\n').as_bytes())
        .expect("sign");

    let hex: String = signature.iter().map(|b| format!("{b:02x}")).collect();
    println!("address:   {}", signer.address());
    println!("signature: 0x{hex}");
}
