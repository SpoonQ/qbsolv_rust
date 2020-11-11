extern crate cc;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
	let out_dir = env::var_os("OUT_DIR").unwrap();
	let dest_dir = Path::new(&out_dir).join("qbsolv");
	let qbsolv_dir = Path::new("contrib").join("qbsolv");

	// mkdir $OUT_DIR/qbsolv
	let _ = fs::remove_dir_all(&dest_dir);
	fs::create_dir_all(&dest_dir).unwrap();

	let mut files = Vec::new();
	// Copy qbsolb/{src,cmd,include}/* to $OUT_DIR/qbsolv
	for subdir in ["src", "cmd", "include"].iter() {
		let src_dir = qbsolv_dir.join(subdir);
		for entry in fs::read_dir(src_dir).unwrap() {
			let entry = entry.unwrap();
			println!("cargo:rerun-if-changed={}", entry.path().to_str().unwrap());
			let dest_file = dest_dir.join(entry.file_name());
			fs::copy(entry.path(), &dest_file).unwrap();
			match dest_file.extension().and_then(|o| o.to_str()) {
				Some("cc") | Some("c") => {
					println!("Using file {:?}", &dest_file);
					files.push(dest_file)
				}
				_ => (),
			}
		}
	}

	// Check is the QOP system is available
	let use_qop = if let Some(dwave_home) = env::var_os("DWAVE_HOME") {
		if Path::new(&dwave_home).join("libepqmi.a").exists() {
			println!("cargo:warning=Using QOP");
			true
		} else {
			println!("cargo:warning='libepqmi.a' is not available in your DWAVE_HOME (='{}'). Using local annealer.", dwave_home.to_str().unwrap());
			false
		}
	} else {
		println!("cargo:warning=DWAVE_HOME not set. Using local annealer.");
		false
	};
	println!("cargo:rerun-if-env-changed=DWAVE_HOME");

	for item in ["dwsolv.cc", "main.c"].iter() {
		let pfstr = format!("{}.patch", item);
		let pforigstr = format!("{}.orig", item);
		let patch_file = Path::new(&pfstr);
		let patched_file = dest_dir.join(item);
		let orig_file = dest_dir.join(&pforigstr);
		fs::rename(&patched_file, &orig_file).unwrap();
		if !orig_file.exists() {
			panic!("orig file {:?} not exists", &orig_file);
		}
		if !patch_file.exists() {
			panic!("patch file {:?} not exists", &patch_file);
		}
		Command::new("patch")
			.arg("-u")
			.arg("-t") // Ask no questions
			.args(&["-o", patched_file.to_str().unwrap()])
			.arg(orig_file.to_str().unwrap())
			.arg(patch_file.to_str().unwrap())
			.status()
			.unwrap();
	}
	// let ar_file = dest_dir.join("libqbsolv.a");
	let mut cc = cc::Build::new();
	let mut cc = files
		.iter()
		.fold(&mut cc, |cc, file| cc.file(file.to_str().unwrap()));
	if use_qop {
		let dwave_home = env::var_os("DWAVE_HOME").unwrap();
		let dwave_home = dwave_home.to_str().unwrap();
		cc = cc
			.flag("-lepqmi")
			.flag(&format!("-L {}", &dwave_home))
			.file(&format!(
				"{}/{}",
				&dwave_home,
				if cfg!(windows) {
					"dwave_sapi.dll"
				} else if cfg!(unix) {
					"libdwave_sapi.so"
				} else if cfg!(target_os = "macos") {
					"libdwave_sapi.dylib"
				} else {
					"/"
				}
			));
		println!("cargo:rustc-cfg=use_qop");
	}
	if cfg!(windows) {
		cc = cc.define("WIN", "true");
	}
	cc.warnings(false)
		.extra_warnings(false)
		.opt_level(3)
		// .flag("-std=gnu99")
		.flag("-lm")
		.shared_flag(true)
		.static_flag(true)
		.define(if use_qop { "QOP" } else { "LOCAL" }, "true") // use external annealer
		.out_dir(&dest_dir)
		.cargo_metadata(true)
		.compile("qbsolv");
	println!("cargo:rerun-if-changed=build.rs");
	// println!("cargo:rustc-link-lib=static={}", "qbsolv");
	// println!(
	// 	"cargo:rustc-link-search=static={}",
	// 	dest_dir.to_str().unwrap()
	// );
	// gcc -Wall -O3 -Wextra -std=gnu99 -I ../src -I ../cmd -I ../include  -D LOCAL -o qbsolv *.c *.cc ../cmd/*.c -lm
}
