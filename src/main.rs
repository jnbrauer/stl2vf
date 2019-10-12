extern crate stl2vf;

use stl2vf::{voxelize, from_stl, write_to_vf};

fn main() {
    let mesh = from_stl("end1x10.stl").expect("Error converting STL");
    println!("Mesh loaded");
    let model = voxelize(&mesh).expect("Error voxelizing model");
    println!("Model voxelized");
    write_to_vf(&model).expect("Error writing VF file");
    println!("VF file written");
}
