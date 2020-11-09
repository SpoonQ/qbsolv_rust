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
	fs::remove_dir_all(&dest_dir).unwrap();
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
	for item in ["dwsolv.cc", "main.c"].iter() {
		let pfstr = format!("{}.patch", item);
		let pforigstr = format!("{}.orig", item);
		let patch_file = Path::new(&pfstr);
		let patched_file = dest_dir.join(item);
		let orig_file = dest_dir.join(&pforigstr);
		fs::rename(&patched_file, &orig_file).unwrap();
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
	files
		.iter()
		.fold(&mut cc, |cc, file| cc.file(file.to_str().unwrap()))
		.warnings(false)
		.extra_warnings(false)
		.opt_level(3)
		// .flag("-std=gnu99")
		.flag("-lm")
		.shared_flag(true)
		.static_flag(true)
		.define("LOCAL", None)
		.out_dir(&dest_dir)
		.compile("qbsolv");
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rustc-link-lib=static={}", "qbsolv");
	println!(
		"cargo:rustc-link-search=static={}",
		dest_dir.to_str().unwrap()
	);
	// gcc -Wall -O3 -Wextra -std=gnu99 -I ../src -I ../cmd -I ../include  -D LOCAL -o qbsolv *.c *.cc ../cmd/*.c -lm
}
