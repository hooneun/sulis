//  This file is part of Sulis, a turn based RPG written in Rust.
//  Copyright 2019 Jared Stephen
//
//  Sulis is free software: you can redistribute it and/or modify
//  it under the terms of the GNU General Public License as published by
//  the Free Software Foundation, either version 3 of the License, or
//  (at your option) any later version.
//
//  Sulis is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License
//  along with Sulis.  If not, see <http://www.gnu.org/licenses/>

use std::io::Error;
use std::rc::Rc;
use std::collections::HashMap;

use sulis_core::util::{Point, ReproducibleRandom};
use crate::{Module, area::{Tile, Layer, GeneratorParams, TransitionBuilder}};
use crate::generator::{WeightedList, WallKinds, RoomParams, TerrainParams, PropParams,
    EncounterParams, GeneratorBuilder, GenModel, Maze, TerrainGen, PropGen, EncounterGen,
    TileKind, TileIter, TilesModel, GeneratorOutput, FeatureParams, FeatureGen,
    TransitionParams, TransitionGen, TransitionOutput};

pub struct AreaGenerator {
    pub id: String,
    wall_kinds: WallKinds,
    grid_width: u32,
    grid_height: u32,
    room_params: RoomParams,
    terrain_params: TerrainParams,
    prop_params: PropParams,
    encounter_params: EncounterParams,
    feature_params: FeatureParams,
    transition_params: TransitionParams,
}

impl AreaGenerator {
    pub fn new(builder: GeneratorBuilder, module: &Module) -> Result<AreaGenerator, Error> {
        let wall_kinds_list = WeightedList::new(builder.wall_kinds, "WallKind",
                                                |id| module.wall_kind(id))?;

        Ok(AreaGenerator {
            id: builder.id,
            wall_kinds: WallKinds { kinds: wall_kinds_list },
            grid_width: builder.grid_width,
            grid_height: builder.grid_height,
            room_params: builder.rooms,
            terrain_params: TerrainParams::new(builder.terrain, module)?,
            prop_params: PropParams::with_module(builder.props, module)?,
            encounter_params: EncounterParams::with_module(builder.encounters, module)?,
            feature_params: FeatureParams::new(builder.features, module)?,
            transition_params: TransitionParams::new(builder.transitions, module)?,
        })
    }

    pub fn generate_transitions(&self, width: i32, height: i32, rand: &mut ReproducibleRandom,
                                params: &GeneratorParams) -> Result<Vec<TransitionOutput>, Error> {

        info!("Generating transitions with rand {:?}", rand);
        let mut gen = TransitionGen::new(width, height, &self.transition_params);
        gen.generate(rand, &params.transitions)
    }

    pub fn generate(&self, width: i32, height: i32, rand: ReproducibleRandom,
                    params: &GeneratorParams,
                    transitions: &[TransitionBuilder],
                    tiles_to_add: Vec<(Rc<Tile>, i32, i32)>) -> Result<GeneratorOutput, Error> {
        info!("Generating area with rand {:?}", rand);

        let mut model = GenModel::new(width, height, rand,
                                      self.grid_width as i32, self.grid_height as i32);

        info!("Model gened {:?}", model.rand());
        let (room_width, room_height) = model.region_size();
        let mut maze = Maze::new(room_width, room_height);

        let open_locs: Vec<Point> = transitions.iter().map(|t| {
            let (x, y) = model.to_region_coords(t.from.x, t.from.y);
            Point::new(x, y)
        }).collect();
        maze.generate(&self.room_params, model.rand_mut(), &open_locs)?;
        info!("Maze generated {:?}", model.rand());

        self.add_walls(&mut model, &maze);

        info!("Generating terrain {:?}", model.rand());
        let mut gen = TerrainGen::new(&mut model, &self.terrain_params, &maze);
        gen.generate();

        for (tile, x, y) in tiles_to_add {
            model.model.add(tile, x, y);
        }
        // add the tiles to the model
        for p in model.tiles() {
            model.model.check_add_wall_border(p.x, p.y);
            model.model.check_add_terrain(p.x, p.y);
            model.model.check_add_terrain_border(p.x, p.y);
        }

        // pre-gen layers for use in the next step
        info!("Tile generation complete.  Pre-Gen layers {:?}", model.rand());
        let layers = self.create_layers(width, height, &model.model)?;

        info!("Generating features {:?}", model.rand());
        let mut gen = FeatureGen::new(&mut model, &layers, &self.feature_params, &maze);
        gen.generate()?;

        info!("Generating props {:?}", model.rand());
        let mut gen = PropGen::new(&mut model, &layers, &self.prop_params, &maze);
        let props = gen.generate(&params.props.passes)?;

        info!("Generating encounters {:?}", model.rand());
        let mut gen = EncounterGen::new(&mut model, &layers, &self.encounter_params, &maze);
        let encounters = gen.generate(&params.encounters.passes)?;

        info!("Final Layer Gen {:?}", model.rand());
        let layers = self.create_layers(width, height, &model.model)?;

        Ok(GeneratorOutput {
            layers,
            props,
            encounters,
        })
    }

    fn create_layers(&self, width: i32, height: i32,
                     model: &TilesModel) -> Result<Vec<Layer>, Error> {
        let mut out = Vec::new();
        for (id, tiles_data) in model.iter() {
            let mut tiles = vec![Vec::new(); (width * height) as usize];
            for (p, tile) in tiles_data.iter() {
                if p.x >= width || p.y >= height { continue; }
                let index = (p.x + p.y * width) as usize;
                tiles[index].push(Rc::clone(tile));
            }

            out.push(Layer::new(width, height, id.to_string(), tiles)?);
        }

        Ok(out)
    }

    fn pick_wall_kind(&self,
                      model: &mut GenModel,
                      invert: bool,
                      region: usize,
                      mapped: &mut HashMap<usize, Option<usize>>) -> (u8, Option<usize>) {
        if !invert { return (0, None); }

        if mapped.contains_key(&region) {
            (1, mapped[&region])
        } else {
            let wall_index = self.wall_kinds.pick_index(&mut model.rand, &model.model);
            mapped.insert(region, wall_index);
            (1, wall_index)
        }
    }

    fn add_walls(&self, model: &mut GenModel, maze: &Maze) {
        // either carve rooms out or put walls in
        if self.room_params.invert {
            for p in model.tiles() {
                model.model.set_wall(p.x, p.y, 0, None);
            }
        } else {
            let wall_index = self.wall_kinds.pick_index(&mut model.rand, &model.model);
            for p in model.tiles() {
                model.model.set_wall(p.x, p.y, 1, wall_index);
            }
        }

        info!("Picked wall type {:?}", model.rand());
        let mut mapped = HashMap::new();

        // carve out procedurally generated rooms
        let room_iter = TileIter::simple(maze.width(), maze.height());
        for p_room in room_iter {
            let region = match maze.region(p_room.x, p_room.y) {
                None => continue,
                Some(region) => region,
            };

            let neighbors = maze.neighbors(p_room.x, p_room.y);
            let (elev, wall_kind) = self.pick_wall_kind(model, self.room_params.invert,
                                                        region, &mut mapped);

            let (offset_x, offset_y) = model.from_region_coords(p_room.x, p_room.y);
            let (tot_gw, tot_gh) = (model.total_grid_size.x, model.total_grid_size.y);
            let (gw, gh) = (model.model.grid_width, model.model.grid_height);
            self.carve_wall(model, neighbors, Point::new(offset_x, offset_y),
                Point::new(gw as i32, gh as i32), Point::new(tot_gw, tot_gh), elev, wall_kind);
        }
    }

    fn carve_wall(&self, model: &mut GenModel, neighbors: [Option<TileKind>; 5],
                  offset: Point, step: Point, max: Point,
                  elev: u8, wall_kind: Option<usize>) {
        let edge_choice = match neighbors[0] {
            Some(TileKind::Corridor(region)) => {
                if model.rand.gen(1, 101) < self.room_params.corridor_edge_overfill_chance {
                    // pregen a single potential overfill for each corridor, preventing
                    // both sides from becoming blocked at room coord intersection
                    Some(*model.region_overfill_edges.entry(region).or_insert(model.rand.gen(1, 5)))
                } else {
                    None
                }
            },
            Some(TileKind::Room { .. }) => {
                if model.rand.gen(1, 101) < self.room_params.room_edge_overfill_chance {
                    Some(model.rand.gen(1, 5))
                } else {
                    None
                }
            },
            _ => None,
        };

        let model = &mut model.model;
        // carve borders with random roughness
        for x in (step.x..max.x - step.x).step_by(step.x as usize) {
            if !is_rough_edge(&neighbors, 1, edge_choice) {
                model.set_wall(x + offset.x, offset.y, elev, wall_kind);
            }
        }

        for y in (step.y..max.y - step.y).step_by(step.y as usize) {
            if !is_rough_edge(&neighbors, 2, edge_choice) {
                model.set_wall(offset.x + max.x - step.x, y + offset.y, elev, wall_kind);
            }
        }

        for x in (step.x..max.x - step.x).step_by(step.x as usize) {
            if !is_rough_edge(&neighbors, 3, edge_choice) {
                model.set_wall(x + offset.x, offset.y + max.y - step.y, elev, wall_kind);
            }
        }

        for y in (step.y..max.y - step.y).step_by(step.y as usize) {
            if !is_rough_edge(&neighbors, 4, edge_choice) {
                model.set_wall(offset.x, y + offset.y, elev, wall_kind);
            }
        }

        // carve corners
        if !is_rough_edge(&neighbors, 1, edge_choice) &&
            !is_rough_edge(&neighbors, 2, edge_choice) {
            model.set_wall(offset.x + max.x - step.x, offset.y, elev, wall_kind);
        }

        if !is_rough_edge(&neighbors, 2, edge_choice) &&
            !is_rough_edge(&neighbors, 3, edge_choice) {
            model.set_wall(offset.x + max.x - step.x, offset.y + max.y - step.y, elev, wall_kind);
        }

        if !is_rough_edge(&neighbors, 3, edge_choice) &&
            !is_rough_edge(&neighbors, 4, edge_choice) {
            model.set_wall(offset.x, offset.y + max.y - step.y, elev, wall_kind);
        }

        if !is_rough_edge(&neighbors, 4, edge_choice) &&
            !is_rough_edge(&neighbors, 1, edge_choice) {
            model.set_wall(offset.x, offset.y, elev, wall_kind);
        }

        // carve center
        for y in (step.y..max.y - step.y).step_by(step.y as usize) {
            for x in (step.x..max.x - step.x).step_by(step.x as usize) {
                model.set_wall(x + offset.x, y + offset.y, elev, wall_kind);
            }
        }
    }
}

fn is_rough_edge(neighbors: &[Option<TileKind>; 5], index: usize,
                 edge_choice: Option<usize>) -> bool {
    if neighbors[index] != Some(TileKind::Wall) { return false; }

    match neighbors[0] {
        Some(TileKind::Room { .. }) => {
            match edge_choice {
                None => false,
                Some(_) => true,
            }
        },
        Some(TileKind::Corridor(_)) => {
            match edge_choice {
                None => false,
                Some(choice) => choice == index
            }
        },
        _ => false,
    }
}
