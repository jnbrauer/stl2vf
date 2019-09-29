extern crate stl2vf;

use stl2vf::Mesh;

fn main() {
    let mesh = Mesh::from_stl("input.stl").expect("Error converting STL");
    let model = mesh.voxelize().expect("Error voxelizing model");
    model.write_to_vf().expect("Error writing VF file");
}
