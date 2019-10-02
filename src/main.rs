extern crate stl2vf;

use stl2vf::Mesh;

fn main() {
    let mesh = Mesh::from_stl("end1x10.stl").expect("Error converting STL");
    println!("Mesh loaded");
    let model = mesh.voxelize().expect("Error voxelizing model");
    println!("Model voxelized");
    model.write_to_vf().expect("Error writing VF file");
    println!("VF file written");
}
