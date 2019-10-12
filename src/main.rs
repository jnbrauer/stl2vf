extern crate stl2vf;

use std::env;
use stl2vf::{voxelize, from_stl, write_to_vf};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Please specify input and output files");
        return;
    }
    let input_file_name = &args[1];
    let output_file_name = &args[2];

    let mesh = from_stl(input_file_name).expect("Error converting STL");
    println!("Mesh loaded");
    let model = voxelize(&mesh).expect("Error voxelizing model");
    println!("Model voxelized");
    write_to_vf(&model, output_file_name).expect("Error writing VF file");
    println!("VF file written");
}
