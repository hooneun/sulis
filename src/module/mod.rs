pub mod actor;
pub use self::actor::Actor;
pub use self::actor::Sex;

pub mod area;
pub use self::area::Area;

pub mod class;
pub use self::class::Class;

pub mod entity_size;
pub use self::entity_size::EntitySize;
pub use self::entity_size::EntitySizeIterator;

pub mod game;
pub use self::game::Game;

mod generator;

pub mod item;
pub use self::item::Item;

pub mod item_adjective;
pub use self::item_adjective::ItemAdjective;

pub mod race;
pub use self::race::Race;

pub mod terrain;
pub use self::terrain::Terrain;

pub mod tile;
pub use self::tile::Tile;

use std::collections::HashMap;
use std::rc::Rc;
use std::io::Error;
use std::cell::RefCell;

use grt::resource::{read, read_single_resource, get_resource, insert_if_ok};

use self::actor::ActorBuilder;
use self::area::AreaBuilder;
use self::class::ClassBuilder;
use self::item::ItemBuilder;
use self::race::RaceBuilder;
use self::entity_size::EntitySizeBuilder;
use self::tile::TileBuilder;

thread_local! {
    static MODULE: RefCell<Module> = RefCell::new(Module::default());
}

pub struct Module {
    game: Option<Rc<Game>>,
    actors: HashMap<String, Rc<Actor>>,
    areas: HashMap<String, Rc<Area>>,
    classes: HashMap<String, Rc<Class>>,
    items: HashMap<String, Rc<Item>>,
    item_adjectives: HashMap<String, Rc<ItemAdjective>>,
    races: HashMap<String, Rc<Race>>,
    sizes: HashMap<usize, Rc<EntitySize>>,
    tiles: HashMap<String, Rc<Tile>>,
}

impl Module {
    pub fn init(root_dir: &str) -> Result<(), Error> {
        let builder_set = ModuleBuilder::new(root_dir);

        debug!("Creating module from parsed data.");

        MODULE.with(|module| {
            let mut module = module.borrow_mut();

            for (id, adj) in builder_set.item_adjectives {
                trace!("Inserting resource of type item_adjective with key {} \
                    into resource set.", id);
                module.item_adjectives.insert(id, Rc::new(adj));
            }

            for (_id_str, builder) in builder_set.size_builders {
                insert_if_ok("size", builder.size, EntitySize::new(builder), &mut module.sizes);
            }

            for (id, builder) in builder_set.tile_builders {
                insert_if_ok("tile", id, Tile::new(builder), &mut module.tiles);
            }

            for (id, builder) in builder_set.item_builders.into_iter() {
                insert_if_ok("item", id, Item::new(builder), &mut module.items);
            }

            for (id, builder) in builder_set.race_builders.into_iter() {
                insert_if_ok("race", id, Race::new(builder, &module), &mut module.races);
            }

            for (id, builder) in builder_set.class_builders.into_iter() {
                insert_if_ok("class", id, Class::new(builder), &mut module.classes);
            }

            for (id, builder) in builder_set.actor_builders.into_iter() {
                insert_if_ok("actor", id, Actor::new(builder, &module), &mut module.actors);
            }

            for (id, builder) in builder_set.area_builders {
                 insert_if_ok("area", id, Area::new(builder, &module), &mut module.areas);
            }
        });

        let game = read_single_resource(&format!("{}/game", root_dir))?;

        MODULE.with(move |m| {
            let mut m = m.borrow_mut();
            m.game = Some(Rc::new(game));
        });

        Ok(())
    }

    pub fn get_actor(id: &str) -> Option<Rc<Actor>> {
        MODULE.with(|r| get_resource(id, &r.borrow().actors))
    }

    pub fn get_area(id: &str) -> Option<Rc<Area>> {
        MODULE.with(|m| get_resource(id, &m.borrow().areas))
    }

    pub fn get_entity_size(id: usize) -> Option<Rc<EntitySize>> {
        MODULE.with(|r| {
            let r = r.borrow();
            let size = r.sizes.get(&id);

            match size {
                None => None,
                Some(s) => Some(Rc::clone(s)),
            }
        })
    }

    pub fn get_all_entity_sizes() -> Vec<Rc<EntitySize>> {
        MODULE.with(|r| r.borrow().sizes.iter().map(|ref s| Rc::clone(s.1)).collect())
    }

    pub fn get_class(id: &str) -> Option<Rc<Class>> {
        MODULE.with(|r| get_resource(id, &r.borrow().classes))
    }

    pub fn get_game() -> Rc<Game> {
        MODULE.with(|m| Rc::clone(m.borrow().game.as_ref().unwrap()))
    }

    pub fn get_race(id: &str) -> Option<Rc<Race>> {
        MODULE.with(|r| get_resource(id, &r.borrow().races))
    }

    pub fn get_tile(id: &str) -> Option<Rc<Tile>> {
        MODULE.with(|r| get_resource(id, &r.borrow().tiles))
    }

    pub fn get_all_tiles() -> Vec<Rc<Tile>> {
        MODULE.with(|r| r.borrow().tiles.iter().map(|ref t| Rc::clone(t.1)).collect())
    }
}

impl Default for Module {
    fn default() -> Module {
        Module {
            game: None,
            actors: HashMap::new(),
            areas: HashMap::new(),
            classes: HashMap::new(),
            items: HashMap::new(),
            item_adjectives: HashMap::new(),
            races: HashMap::new(),
            sizes: HashMap::new(),
            tiles: HashMap::new(),
        }
    }
}

struct ModuleBuilder {
    actor_builders: HashMap<String, ActorBuilder>,
    area_builders: HashMap<String, AreaBuilder>,
    class_builders: HashMap<String, ClassBuilder>,
    item_builders: HashMap<String, ItemBuilder>,
    item_adjectives: HashMap<String, ItemAdjective>,
    race_builders: HashMap<String, RaceBuilder>,
    size_builders: HashMap<String, EntitySizeBuilder>,
    tile_builders: HashMap<String, TileBuilder>,
}

impl ModuleBuilder {
    fn new(root_dir: &str) -> ModuleBuilder {
        ModuleBuilder {
            actor_builders: read(root_dir, "actors"),
            area_builders: read(root_dir, "areas"),
            class_builders: read(root_dir, "classes"),
            item_builders: read(root_dir, "items"),
            item_adjectives: read(root_dir, "item_adjectives"),
            race_builders: read(root_dir, "races"),
            size_builders: read(root_dir, "sizes"),
            tile_builders: read(root_dir, "tiles"),
        }
    }
}
