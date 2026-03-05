use std::env;
use std::fs;
use std::path::Path;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set("CompanyName", "David CM <dhf360@gmail.com>");
        res.set("FileDescription", "32-bit host using Named Pipes and shared memory for IBM TTS (eci)");
        res.set("ProductName", "IBM TTS host 32 Bridge");
        res.set("OriginalFilename", "ibmtts_host32.dll");
        res.set("LegalCopyright", "Copyright © 2026 David CM. Licensed under GNU GPL v2.");
        res.set("Comments", "This is open source free software; Source code available at: https://github.com/davidacm/ibmtts-host32-bridge");
        res.set("ProjectURL", "https://github.com/davidacm/ibmtts-host32-bridge");

        // Version: "1.0.0.0" -> The format must be u64: 0xMMmmppppRRrr
        res.set_version_info(winres::VersionInfo::FILEVERSION, 0x0001000000000000);

        res.set_manifest_file("app.manifest");

        res.compile().expect("Could not compile Windows resources");
    }

    let api_files = vec![
        "src/worker.rs",
    ];

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("api_dispatch.rs");
    let mut match_arms = String::new();

    for file_path in api_files {
        println!("cargo:rerun-if-changed={}", file_path);
        let content = fs::read_to_string(file_path)
            .expect("Could not read API file");

        let lines: Vec<&str> = content.lines().collect();
        
        for i in 0..lines.len() {
            let line = lines[i].trim();
            
            // look for the brand #[api(N)]
            if line.starts_with("#[api(") && line.contains(')') {
                // Extract the ID: #[api(123)] -> 123
                let id = line.split('(').nth(1)
                             .and_then(|s| s.split(')').next())
                             .unwrap_or("")
                             .trim();

                if id.is_empty() { continue; }

                // look for the function in the next 3 lines
                let mut found = false;
                for j in (i + 1)..(i + 4).min(lines.len()) {
                    let next_line = lines[j].trim();
                    
                    if next_line.contains("fn ") {
                        // Extract name: "fn eci_stop(..." -> "eci_stop"
                        // split by "fn", take what follows, and then cut at "("
                        if let Some(after_fn) = next_line.split("fn ").nth(1) {
                            let fn_name = after_fn.split('(').next().unwrap().trim();
                            match_arms.push_str(&format!("        {} => {}(&ctx),\n", id, fn_name));
                            found = true;
                            break;
                        }
                    }
                }
                
                if !found {
                    panic!("Error: #[api({})] was found but the function 'fn' underneath was not detected.", id);
                }
            }
        }
    }

    // Generating the final match
    let code = format!(
"match id {{
{}
    _ => {{
        let mut v = b\"ERR:unknown id \".to_vec();
        v.extend_from_slice(&id.to_le_bytes());
        pack_bytes(v)
    }}
}}", match_arms);

    fs::write(&dest_path, code).expect("Could not write api_dispatch.rs");
}