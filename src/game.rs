use log::info;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

use crate::error::Error;

use self::pieces::Direction;

mod drawer;
mod field;
mod gen;
mod pieces;
mod size;
mod theme;

pub fn load_default(ctx: &rlua::Context) -> Result<(), Error>
{
	ctx.load(
r#"pieces={[1]={size=4,template=[[0000111100000000]],color={r=68,g=210,b=242,a=0xFF,},},[2]={size=3,template=[[100111000]],color={r=53,g=39,b=145,a=0xFF,},},[3]={size=3,template=[[001111000]],color={r=227,g=133,b=61,a=0xFF,},},[4]={size=2,template=[[1111]],color={r=242,g=210,b=68,a=0xFF,},},[5]={size=3,template=[[011110000]],color={r=49,g=186,b=47,a=0xFF,},},[6]={size=3,template=[[010111000]],color={r=142,g=47,b=186,a=0xFF,},},[7]={size=3,template=[[110011000]],color={r=196,g=47,b=47,a=0xFF,},}}function spawn_piece()local r=math.random(1,#(pieces))return pieces[r]end function init_game()return{width=10,height=20}end math.randomseed(os.time())"#
	)
	.exec()?;

	Ok(())
}

// -----------------------------------------------------------------------------
// Game State
// -----------------------------------------------------------------------------

pub struct TetrisState<'a, 'b, 'c>
{
	// Field
	field_blocks: Vec<Point>,
	field_colors: Vec<Color>,
	field_size: (usize, usize),

	// Piece
	piece_proj: i32,
	piece_loc: Point,
	piece_dim: usize,
	piece_blocks: Vec<Point>,
	piece_colors: Vec<Color>,

	// Drawer
	rblock_size: u32,
	rfield_rect: Rect,
	rblocks_texture: Texture<'a>,

	// Lua context
	lua_ctx: &'c rlua::Context<'b>,

	// Stats
	score: u64,
}

impl<'a, 'b, 'c> TetrisState<'a, 'b, 'c>
{
	pub fn init(
		tc: &'a TextureCreator<WindowContext>,
		dim: (u32, u32),
		lua_ctx: &'c rlua::Context<'b>,
	) -> Result<Self, Error>
	{
		let t = theme::load(lua_ctx)?;

		let (field_blocks, field_colors, field_size) = field::init(t.field_dim);

		let p = gen::spawn_piece(&lua_ctx)?;

		let (piece_blocks, piece_colors, piece_dim, piece_loc, piece_proj) =
			if let Some(d) = pieces::init(p, field_size, &field_blocks) {
				d
			} else {
				return Err(Error::from("Piece couldn't be spawned."));
			};

		let p = size::new_resize(dim, field_size);
		let (rblock_size, rfield_rect, rblocks_texture) = drawer::init(tc, p);

		let score = 0;

		Ok(TetrisState {
			field_blocks,
			field_colors,
			field_size,
			piece_proj,
			piece_loc,
			piece_dim,
			piece_blocks,
			piece_colors,
			rblock_size,
			rfield_rect,
			rblocks_texture,
			lua_ctx,
			score,
		})
	}
}

impl TetrisState<'_, '_, '_>
{
	fn respawn_piece(&mut self) -> Result<bool, Error>
	{
		info!("Respawning piece.");

		let fb = &self.field_blocks;
		let fs = self.field_size;
		let l = &self.lua_ctx;

		let p = gen::spawn_piece(&l)?;

		if let Some((pb, pc, pd, pp, pj)) = pieces::spawn_new(p, fs, fb) {
			self.piece_blocks = pb;
			self.piece_colors = pc;
			self.piece_dim = pd;
			self.piece_loc = pp;
			self.piece_proj = pj;
			return Ok(true);
		}

		Ok(false)
	}

	fn place_piece(&mut self, canvas: &mut WindowCanvas)
	{
		info!("Placing piece.");

		let fb = &mut self.field_blocks;
		let fc = &mut self.field_colors;
		let pb = &self.piece_blocks;
		let pc = &self.piece_colors;
		let pl = self.piece_loc;

		fb.extend(pb.iter().map(|b| Point::new(b.x + pl.x, b.y + pl.y)));
		fc.extend(pc);

		let fs = self.field_size;
		let bs = self.rblock_size;

		field::clear_lines(fs, fb, fc);

		let ft = &mut self.rblocks_texture;

		canvas
			.with_texture_canvas(ft, |canvas| {
				canvas.set_draw_color(Color::RGBA(0, 0, 0, 0));
				canvas.clear();

				drawer::draw_blocks(canvas, bs, &fb, &fc);
			})
			.unwrap();
	}

	fn drop(&mut self, canvas: &mut WindowCanvas) -> Result<bool, Error>
	{
		info!("Dropping piece.");

		let pj = self.piece_proj;

		self.piece_loc.y = pj;

		self.place_piece(canvas);
		self.respawn_piece()
	}

	fn rotate(&mut self)
	{
		let fb = &self.field_blocks;
		let fs = self.field_size;
		let pb = &self.piece_blocks;
		let pl = self.piece_loc;
		let pd = self.piece_dim;

		let new_pb: Vec<Point> = pb.iter().map(|b| Point::new(pd as i32 - 1 - b.y, b.x)).collect();

		if field::check_valid_pos(fs, fb, pl, &new_pb) {
			info!("Rotating piece.");

			let p = pieces::project(fs, fb, pl, &new_pb);
			self.piece_blocks = new_pb;
			self.piece_proj = p;
		}
	}

	fn move_piece(&mut self, d: Direction)
	{
		let fb = &self.field_blocks;
		let fs = self.field_size;
		let pb = &self.piece_blocks;
		let pl = self.piece_loc;

		if let Some((pl, proj)) = pieces::move_piece(fs, fb, pl, pb, d) {
			self.piece_loc = pl;
			self.piece_proj = proj;
		}
	}

	fn move_piece_down(&mut self, canvas: &mut WindowCanvas) -> Result<bool, Error>
	{
		let fb = &self.field_blocks;
		let fs = self.field_size;
		let pb = &self.piece_blocks;
		let pl = self.piece_loc;

		if let Some((pl, proj)) = pieces::move_piece(fs, fb, pl, pb, Direction::DOWN) {
			self.piece_loc = pl;
			self.piece_proj = proj;
			return Ok(false);
		}

		self.place_piece(canvas);
		self.respawn_piece()
	}

	fn draw_field(&self, canvas: &mut WindowCanvas)
	{
		let fr = self.rfield_rect;
		let bt = &self.rblocks_texture;

		canvas.set_draw_color(Color::BLACK);
		canvas.fill_rect(fr).unwrap();

		canvas.copy(bt, None, fr).unwrap();
	}

	pub fn draw_player(&self, canvas: &mut WindowCanvas)
	{
		let pcs = &self.piece_colors;
		let pbs = &self.piece_blocks;
		let fr = self.rfield_rect;
		let bs = self.rblock_size;
		let pos = self.piece_loc;
		let proj = self.piece_proj;

		debug_assert_eq!(pcs.len(), pbs.len());

		for (c, b) in pcs.iter().zip(pbs) {
			let color = Color::RGBA(c.r, c.g, c.b, c.a / 2);
			let block = Rect::new(
				fr.x + (b.x + pos.x) * bs as i32,
				fr.y + (b.y + proj) * bs as i32,
				bs,
				bs,
			);

			canvas.set_draw_color(color);
			canvas.fill_rect(block).unwrap();

			let color = *c;
			let block = Rect::new(
				fr.x + (b.x + pos.x) * bs as i32,
				fr.y + (b.y + pos.y) * bs as i32,
				bs,
				bs,
			);

			canvas.set_draw_color(color);
			canvas.fill_rect(block).unwrap();
		}
	}

	pub fn output_score(&self)
	{
		println!("Well done! Your score is {}.", self.score);
	}
}

// -----------------------------------------------------------------------------
// Game
// -----------------------------------------------------------------------------

pub fn handle_event(
	event: &Event,
	canvas: &mut WindowCanvas,
	state: &mut TetrisState,
) -> Result<bool, Error>
{
	match event {
		Event::KeyDown {
			keycode: Some(x), ..
		} => match x {
			Keycode::Left => {
				state.move_piece(Direction::LEFT);
			}

			Keycode::Right => {
				state.move_piece(Direction::RIGHT);
			}

			Keycode::Down => {
				return Ok(state.move_piece_down(canvas)?);
			}

			Keycode::Up => {
				state.rotate();
			}

			Keycode::Space => {
				return Ok(state.drop(canvas)?);
			}

			_ => (),
		},

		Event::Window {
			win_event: WindowEvent::Resized(w, h),
			..
		} => {
			// set_layout_size(&mut self.draw_cache, &self.field, (*w as u32, *h as u32));
		}

		_ => (),
	}

	Ok(true)
}

pub fn draw(state: &TetrisState, canvas: &mut WindowCanvas)
{
	state.draw_field(canvas);
	state.draw_player(canvas);
}
