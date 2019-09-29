use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;

use ndarray::{Array1, Array2, Array3};
use ndarray_linalg::*;

pub struct VoxelModel {
    voxels: Array3<i8>,
    x_len: usize,
    y_len: usize,
    z_len: usize
}

impl VoxelModel {
    pub fn write_to_vf(&self) -> std::io::Result<()> {
        let mut file = File::create("output.vf")?;

        // Coordinates
        writeln!(file, "<coords>\n0,0,0,\n</coords>")?;

        // Materials
        writeln!(file, "<materials>")?;
        writeln!(file, "0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,")?;
        writeln!(file, "1.0,0.0,1.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,")?;
        writeln!(file, "</materials>")?;

        // Size
        writeln!(file, "<size>\n{},{},{},\n</size>", self.x_len, self.y_len, self.z_len)?;

        // Voxels
        writeln!(file, "<voxels>")?;
        for x in 0..self.x_len {
            for z in 0..self.z_len {
                for y in 0..self.y_len {
                    write!(file, "{},", self.voxels[[x, y, z]])?;
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
}

pub struct Mesh {
    points: Array2<f32>,
    tets: Array2<i32>
}

impl Mesh {
    pub fn from_stl(filename: &str) -> Result<Mesh, Box<dyn Error>> {
        // Use gmsh to convert STL file to mesh file
        Command::new("gmsh")
            .arg("output.geo")
            .arg("-3")
            .arg("-format")
            .arg("msh")
            .output()
            .expect("Gmsh execution failed");

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

        let nodes_info_line = utils::split_string(&lines[nodes_start+1]);
        let point_blocks = nodes_info_line[0].parse()?;

        line_number = nodes_start + 2;
        for _ in 0..point_blocks {
            let block_info_line = utils::split_string(&lines[line_number]);
            let n = block_info_line[3].parse()?;
            line_number += n + 1 as usize;
            for _ in 0..n {
                let point_line = utils::split_string(&lines[line_number]);
                let x: f32 = point_line[0].parse()?;
                let y: f32 = point_line[1].parse()?;
                let z: f32 = point_line[2].parse()?;

                points.extend_from_slice(&vec![x, y, z]);
                n_points += 1;

                line_number += 1;
            }
        }

        let tris_info_line = utils::split_string(&lines[elements_start+2]);
        let n_tris: usize = tris_info_line[3].parse()?;

        line_number = elements_start + n_tris + 3;
        let tets_info_line = utils::split_string(&lines[line_number]);
        let n_tets = tets_info_line[3].parse()?;

        line_number += 1;
        for _ in 0..n_tets {
            let point_line = utils::split_string(&lines[line_number]);
            let a: i32 = point_line[1].parse()?;
            let b: i32 = point_line[2].parse()?;
            let c: i32 = point_line[3].parse()?;
            let d: i32 = point_line[4].parse()?;

            tets.extend_from_slice(&vec![a-1, b-1, c-1, d-1]);

            line_number += 1;
        }

        let points = Array2::from_shape_vec((n_points, 3), points)?;
        let tets = Array2::from_shape_vec((n_tets, 4), tets)?;

        return Ok(Mesh {points, tets});
    }

    pub fn voxelize(&self) -> Result<VoxelModel, Box<dyn Error>> {
        let mut x_min = self.points[[0, 0]];
        let mut x_max = self.points[[0, 0]];
        let mut y_min = self.points[[0, 1]];
        let mut y_max = self.points[[0, 1]];
        let mut z_min = self.points[[0, 2]];
        let mut z_max = self.points[[0, 2]];

        for point in self.points.genrows() {
            if point[0] < x_min { x_min = point[0]; }
            if point[0] > x_max { x_max = point[0]; }

            if point[1] < y_min { y_min = point[1]; }
            if point[1] > y_max { y_max = point[1]; }

            if point[2] < z_min { z_min = point[2]; }
            if point[2] > z_max { z_max = point[2]; }
        }

        let x_min = x_min.round() as i32;
        let x_max = x_max.round() as i32;
        let y_min = y_min.round() as i32;
        let y_max = y_max.round() as i32;
        let z_min = z_min.round() as i32;
        let z_max = z_max.round() as i32;

        let x_len: usize = (x_max - x_min) as usize;
        let y_len: usize = (y_max - y_min) as usize;
        let z_len: usize = (z_max - z_min) as usize;
        let n_voxels = x_len * y_len * z_len;

        let mut grid_xyz: Array2<f32> = Array2::zeros((n_voxels, 4));
        let mut grid_ijk: Array2<usize> = Array2::zeros((n_voxels, 3));

        let x_start: f32 = x_min as f32 + 0.5;
        let y_start: f32 = y_min as f32 + 0.5;
        let z_start: f32 = z_min as f32 + 0.5;

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

        let mut model: Array3<i8> = Array3::zeros((x_len, y_len, z_len));
        for tet in self.tets.genrows() {
            let mut tet_full = Array2::zeros((4, 4));
            for i in 0..4 {
                for j in 0..3 {
                    tet_full[[i, j]] = self.points[[tet[i] as usize, j]];
                }
                tet_full[[i, 3]] = 1.0;
            }

            let mut inverse = tet_full.inv()?;
            inverse = inverse.t().to_owned();

            for i in 0..n_voxels {
                let x = grid_ijk[[i, 0]];
                let y = grid_ijk[[i, 1]];
                let z = grid_ijk[[i, 2]];

                if model[[x, y, z]] != 1 {
                    let mut dot_products: Array1<f32> = Array1::zeros(4);
                    for j in 0..4 {
                        dot_products[j] = inverse.row(j).dot(&grid_xyz.row(i));
                    }

                    if utils::all_in_range(&dot_products, 0.0, 1.0) { // check if point is inside tet
                        model[[x, y, z]] = 1;
                    }
                }
            }
        }

        return Ok(VoxelModel {
            voxels: model,
            x_len,
            y_len,
            z_len
        });
    }
}

mod utils {
    use ndarray::Array1;

    pub fn split_string(s: &str) -> Vec<&str> {
        let parts_iterator = s.split_whitespace();
        let mut parts: Vec<&str> = Vec::new();

        parts_iterator.for_each(|part| parts.push(part));

        return parts;
    }

    pub fn all_in_range(array: &Array1<f32>, low: f32, high: f32) -> bool {
        for i in array.iter() {
            if *i < low-0.00000000001 || *i > high+0.00000000001 {
                return false;
            }
        }

        return true;
    }
}
