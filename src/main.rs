// src/main.rs
use ibmtts_host32; // El nombre de tu paquete en Cargo.toml

fn main() {
    // Llama a una función pública que expongas en lib.rs
    ibmtts_host32::run_host(); 
}