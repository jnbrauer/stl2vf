use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write, BufWriter};
use std::path::Path;
use std::process::Command;
use std::thread;

use ndarray::{Array1, Array2, Array3};
use ndarray_linalg::*;
use std::sync::{Arc, Mutex};

pub struct VoxelModel {
    voxels: Array3<u8>,
    x_len: usize,
    y_len: usize,
    z_len: usize
}

#[derive(Clone)]
pub struct Mesh {
    points: Array2<f32>,
    tets: Array2<i32>
}

pub fn write_to_vf(model: &VoxelModel, file_name: &str) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(file_name)?);

    // Coordinates
    writeln!(file, "<coords>\n0,0,0,\n</coords>")?;

    // Materials
    writeln!(file, "<materials>")?;
    writeln!(file, "0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,")?;
    writeln!(file, "1.0,0.0,1.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,")?;
    writeln!(file, "</materials>")?;

    // Size
    writeln!(file, "<size>\n{},{},{},\n</size>", model.x_len, model.y_len, model.z_len)?;

    // Voxels
    writeln!(file, "<voxels>")?;
    for x in 0..model.x_len {
        for z in 0..model.z_len {
            for y in 0..model.y_len {
                write!(file, "{},", model.voxels[[x, y, z]])?;
            }
            write!(file, ";")?;
        }
        writeln!(file)?;
    }
    writeln!(file, "</voxels>")?;

    // Components
    writeln!(file, "<components>\n0\n</components>")?;

    return Ok(());
}

pub fn from_stl(filename: &str) -> Result<Mesh, Box<dyn Error>> {
    let mut gmsh_script_file = File::create("output.geo")?;

    writeln!(gmsh_script_file, "Merge \"{}\";", filename)?;
    writeln!(gmsh_script_file, "Surface Loop(1) = {{1}};")?;
    writeln!(gmsh_script_file, "//+")?;
    writeln!(gmsh_script_file, "Volume(1) = {{1}};")?;

    // Use gmsh to convert STL file to mesh file
    Command::new("gmsh")
        .arg("output.geo")
        .arg("-3")
        .arg("-format")
        .arg("msh")
        .output()?;

    // List of points
    let mut points: Vec<f32> = Vec::new();
    // List of tetrahedrons
    let mut tets: Vec<i32> = Vec::new();

    let mut n_points = 0;

    // Open msh file created by gmsh
    let file_path = Path::new("output.msh");
    let file = File::open(&file_path)?;

    // Current line number being processed
    let mut line_number;

    // Line number of the start of the nodes block
    let mut nodes_start = 0;
    // Line number of the start of the elements block
    let mut elements_start = 0;

    let mut lines: Vec<String> = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?.trim().to_owned();

        if &line == "$Nodes" {
            nodes_start = lines.len();
        } else if &line == "$Elements" {
            elements_start = lines.len();
        }

        lines.push(line);
    }

    let nodes_info_line = split_string(&lines[nodes_start+1]);
    let point_blocks = nodes_info_line[0].parse()?;

    line_number = nodes_start + 2;
    for _ in 0..point_blocks {
        let block_info_line = split_string(&lines[line_number]);
        let n = block_info_line[3].parse()?;
        line_number += n + 1 as usize;
        for _ in 0..n {
            let point_line = split_string(&lines[line_number]);
            let x: f32 = point_line[0].parse()?;
            let y: f32 = point_line[1].parse()?;
            let z: f32 = point_line[2].parse()?;

            points.extend_from_slice(&vec![x, y, z]);
            n_points += 1;

            line_number += 1;
        }
    }

    let tris_info_line = split_string(&lines[elements_start+2]);
    let n_tris: usize = tris_info_line[3].parse()?;

    line_number = elements_start + n_tris + 3;
    let tets_info_line = split_string(&lines[line_number]);
    let n_tets = tets_info_line[3].parse()?;

    line_number += 1;
    for _ in 0..n_tets {
        let point_line = split_string(&lines[line_number]);
        let a: i32 = point_line[1].parse()?;
        let b: i32 = point_line[2].parse()?;
        let c: i32 = point_line[3].parse()?;
        let d: i32 = point_line[4].parse()?;

        tets.extend_from_slice(&vec![a-1, b-1, c-1, d-1]);

        line_number += 1;
    }

    let points = Array2::from_shape_vec((n_points, 3), points)?;
    let tets = Array2::from_shape_vec((n_tets, 4), tets)?;

    Command::new("rm").arg("output.geo").spawn()?;
    Command::new("rm").arg("output.msh").spawn()?;

    return Ok(Mesh {points, tets});
}

pub fn voxelize(mesh: &Mesh) -> Result<VoxelModel, Box<dyn Error>> {
    let mesh = mesh.clone();

    // Get min and max values in each axis
    let mut x_min = mesh.points[[0, 0]];
    let mut x_max = mesh.points[[0, 0]];
    let mut y_min = mesh.points[[0, 1]];
    let mut y_max = mesh.points[[0, 1]];
    let mut z_min = mesh.points[[0, 2]];
    let mut z_max = mesh.points[[0, 2]];

    for point in mesh.points.genrows() {
        if point[0] < x_min { x_min = point[0]; }
        if point[0] > x_max { x_max = point[0]; }

        if point[1] < y_min { y_min = point[1]; }
        if point[1] > y_max { y_max = point[1]; }

        if point[2] < z_min { z_min = point[2]; }
        if point[2] > z_max { z_max = point[2]; }
    }

    // Round min and max values to integers
    let x_min = x_min.round() as i32;
    let x_max = x_max.round() as i32;
    let y_min = y_min.round() as i32;
    let y_max = y_max.round() as i32;
    let z_min = z_min.round() as i32;
    let z_max = z_max.round() as i32;

    // Calculate x, y, and z lengths
    let x_len = (x_max - x_min) as usize;
    let y_len = (y_max - y_min) as usize;
    let z_len = (z_max - z_min) as usize;
    // Calculate total number of voxels
    let n_voxels = x_len * y_len * z_len;

    // Create array to store coordinates of each voxel
    let mut grid_xyz: Array2<f32> = Array2::zeros((n_voxels, 4));
    // Create array to store position of each voxel in grid
    let mut grid_ijk: Array2<usize> = Array2::zeros((n_voxels, 3));

    // Find coordinates of center of first voxel
    let x_start: f32 = x_min as f32 + 0.5;
    let y_start: f32 = y_min as f32 + 0.5;
    let z_start: f32 = z_min as f32 + 0.5;

    // Generate list of voxels
    let mut row = 0;
    for i in 0..x_len {
        for j in 0..y_len {
            for k in 0..z_len {
                let mut row_xyz = grid_xyz.row_mut(row);
                row_xyz[0] = x_start + i as f32;
                row_xyz[1] = y_start + j as f32;
                row_xyz[2] = z_start + k as f32;
                row_xyz[3] = 1.0;

                let mut row_ijk = grid_ijk.row_mut(row);
                row_ijk[0] = i;
                row_ijk[1] = j;
                row_ijk[2] = k;

                row += 1;
            }
        }
    }

    // Create complete voxel grid
    let model: Arc<Mutex<Array3<u8>>> = Arc::new(Mutex::new(Array3::zeros((x_len, y_len, z_len))));
    let grid_ijk: Arc<Array2<usize>> = Arc::new(grid_ijk);
    let grid_xyz: Arc<Array2<f32>> = Arc::new(grid_xyz);
    let points: Arc<Array2<f32>> = Arc::new(mesh.points);

    let mut thread_handles = vec![];

    for tet in mesh.tets.genrows() {
        let tet = tet.to_owned();

        let model = Arc::clone(&model);
        let grid_ijk = Arc::clone(&grid_ijk);
        let grid_xyz = Arc::clone(&grid_xyz);
        let points = Arc::clone(&points);

        let handle = thread::spawn(move || {
            let mut tet_full = Array2::zeros((4, 4));
            for i in 0..4 {
                for j in 0..3 {
                    tet_full[[i, j]] = points[[tet[i] as usize, j]];
                }
                tet_full[[i, 3]] = 1.0;
            }

            let mut inverse = tet_full.inv().unwrap();
            inverse = inverse.t().to_owned();

            let mut filled_voxels = Vec::with_capacity(n_voxels);
            for i in 0..n_voxels {
                let x = grid_ijk[[i, 0]];
                let y = grid_ijk[[i, 1]];
                let z = grid_ijk[[i, 2]];
                let point = vec![x, y, z];

                let mut dot_products: Array1<f32> = Array1::zeros(4);
                for j in 0..4 {
                    dot_products[j] = inverse.row(j).dot(&grid_xyz.row(i));
                }

                if all_in_range(&dot_products, 0.0, 1.0) { // check if point is inside tet
                    filled_voxels.push(point);
                }
            }

            let mut model = model.lock().unwrap();
            for point in filled_voxels {
                model[[point[0], point[1], point[2]]] = 1;
            }
        });
        thread_handles.push(handle);
    }

    for handle in thread_handles {
        handle.join().unwrap();
    }

    return Ok(VoxelModel {
        voxels: model.lock().unwrap().to_owned(),
        x_len,
        y_len,
        z_len
    });
}

fn split_string(s: &str) -> Vec<&str> {
    let parts_iterator = s.split_whitespace();
    let mut parts: Vec<&str> = Vec::new();

    parts_iterator.for_each(|part| parts.push(part));

    return parts;
}

fn all_in_range(array: &Array1<f32>, low: f32, high: f32) -> bool {
    for i in array.iter() {
        if *i < low-0.00000000001 || *i > high+0.00000000001 {
            return false;
        }
    }

    return true;
}
