extern crate find_folder;
extern crate freetype;
extern crate image;
extern crate piston_window;

use image::*;
use piston_window::*;
use std::env;
use std::time::SystemTime;

mod cpu;
mod instruction;
mod machine;
mod ppu;
mod system;

const DEBUG_WIDTH: u32 = 600;
const DEBUG_HEIGHT: u32 = 0; // 100;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;
const SIZE: f64 = 2.0; // !you must change to 2 if use gdb

fn main() {
  // デバッグモード判定用
  let args: Vec<String> = env::args().collect();

  let gui_debug = args.contains(&"gdebug".to_string())
    || args.contains(&"gdb".to_string())
    || args.contains(&"g".to_string());

  let cui_debug = args.contains(&"cdebug".to_string())
    || args.contains(&"cdb".to_string())
    || args.contains(&"c".to_string());

  // 初期化する
  let mut machine = machine::Machine::new();
  let mut cpu = cpu::Cpu::new();
  let mut ppu = ppu::Ppu::new();

  // カセット読み込み
  let path = "./roms/sample1.nes"; // Hello World (未達成)
                                   // let path = "./roms/nestest.nes"; // 色々テストできるROM
                                   // let path = "./roms/SHOOT.nes"; // シューティングゲーム (未達成)
  let result = system::load_cassette(&mut machine, path.to_string(), cui_debug); // ROMのサイズをmachineに渡す

  let (prg_rom, chr_rom) = match result {
    Ok(rom) => rom,
    Err(_) => panic!("Failed to get PRG-ROM or CHR-ROM"),
  };

  // machineにROMをセット
  machine.set_roms(prg_rom, chr_rom);

  // 電源が入るとRESETの割込処理が走る
  cpu.interrupt(&mut machine, instruction::Interrupt::RESET);

  // GUI
  let opengl = OpenGL::V3_2;
  let width = WIDTH * SIZE as u32 + if gui_debug { DEBUG_WIDTH } else { 0 };
  let height = HEIGHT * SIZE as u32 + if gui_debug { DEBUG_HEIGHT } else { 0 };
  let mut window: PistonWindow =
    WindowSettings::new(format!("NES Emulator ({})", path), (width, height))
      .graphics_api(opengl)
      .exit_on_esc(true)
      .build()
      .expect("Failed to build window.");

  // フォントの読み込み
  let assets = find_folder::Search::ParentsThenKids(3, 3)
    .for_folder("assets")
    .unwrap();
  let ref font = assets.join("Arimo for Powerline.ttf");

  let factory = window.create_texture_context();
  let mut glyphs = Glyphs::new(font, factory, TextureSettings::new()).unwrap(); // thanks, @megumish

  // NESの画面
  let mut screen = ImageBuffer::new(WIDTH * SIZE as u32, HEIGHT * SIZE as u32);

  let mut texture_context = TextureContext {
    factory: window.factory.clone(),
    encoder: window.factory.create_command_buffer().into(),
  };

  let mut texture = Texture::from_image(&mut texture_context, &screen, &TextureSettings::new())
    .expect("Failed to create texture.");

  // デバッグ側にCHR-ROMを書き出す画面
  let mut debug_screen = ImageBuffer::new(WIDTH * SIZE as u32 + DEBUG_WIDTH, HEIGHT * SIZE as u32);

  let mut debug_texture_context = TextureContext {
    factory: window.factory.clone(),
    encoder: window.factory.create_command_buffer().into(),
  };

  let mut debug_texture = Texture::from_image(
    &mut debug_texture_context,
    &debug_screen,
    &TextureSettings::new(),
  )
  .expect("Failed to create texture.");

  let start_at = SystemTime::now(); // システムの起動時間を計測
  let timing = (263 * (341 / 3)) as usize; // PPUと同期するために必要
  let mut cpu_count = 0; // 命令の実行数をカウント
  let mut max_pc: u16 = 0; // デバッグ用 pc最大値

  let mut events = Events::new(EventSettings::new());
  while let Some(e) = events.next(&mut window) {
    if let Some(args) = e.render_args() {
      // タイミング調整用
      let mut cycles = 0;
      while cycles < timing {
        let exec_res = cpu.exec(&mut machine);
        // println!("inst {:x}", exec_res.1);
        cpu_count += 1;

        cycles += exec_res.0 as usize;
        if max_pc < cpu.pc {
          max_pc = cpu.pc;
          println!("max pc {:<02}, opecode {:?}", max_pc, cpu.convert(exec_res.1));
        }
      }

      // PPUでアレコレしてNESの画面を更新
      texture.update(&mut texture_context, &screen).unwrap();

      window.draw_2d(&e, |c, g, d| {
        clear([0.0, 0.0, 0.0, 1.0], g);
        texture_context.encoder.flush(d);
        image(&texture, c.transform.scale(SIZE, SIZE), g);

        if gui_debug {
          // デバッグ用の背景を右側に描画する
          rectangle(
            [0.0, 0.0, 0.5, 1.0],
            [
              WIDTH as f64 * SIZE + 1.0,
              0.0,
              DEBUG_WIDTH as f64 * SIZE,
              height as f64,
            ], // x, y, w, h
            c.transform,
            g,
          );

          // フラグの状態
          let margin_x = 40.0;
          let base_y = 360.0;

          let mut text = "Flags".to_string();
          let mut transform = c.transform.trans(WIDTH as f64 * SIZE + margin_x, base_y);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          text = "N V - B D I Z C".to_string();
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x + 50.0, base_y - 23.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          for i in 0..8 {
            let digit = (7 - i);
            let stat = cpu.p & (1 << digit) == 0;
            let color = if stat {
              [0.1, 0.9, 0.6, 1.0]
            } else {
              [0.9, 0.1, 0.3, 1.0]
            };

            text = (if stat { "▲" } else { "▼" }).to_string();
            transform = c.transform.trans(
              WIDTH as f64 * SIZE + margin_x + 47.0 + i as f64 * 15.5,
              base_y,
            );
            text::Text::new_color(color, 15)
              .draw(&text, &mut glyphs, &c.draw_state, transform, g)
              .unwrap();
          }

          // レジスタの状態
          text = format!("A: 0x{:<08x}", cpu.a);
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x, base_y + 30.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          text = format!("X: 0x{:<08x}", cpu.x);
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x, base_y + 60.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          text = format!("Y: 0x{:<08x}", cpu.y);
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x, base_y + 90.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          text = format!("SP: 0x{:<08x}", cpu.sp);
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x, base_y + 120.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          text = format!("PC: 0x{:<016x}", cpu.pc);
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x, base_y + 150.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          // 起動時間
          text = format!(
            "Startup time: {:<010}(s)",
            match start_at.elapsed() {
              Ok(elapsed) => elapsed.as_secs(),
              Err(_) => panic!(),
            }
          );
          transform = c
            .transform
            .trans(WIDTH as f64 * SIZE + margin_x, base_y + 180.0);
          text::Text::new_color([1.0; 4], 15)
            .draw(&text, &mut glyphs, &c.draw_state, transform, g)
            .unwrap();

          // キャッシュクリアしたりいい感じにする
          glyphs.factory.encoder.flush(d);

          // 直接CHR-ROMの中身を全部描画してみる
          for i in 0..chr_rom.len() / 16
          /* (32 * 10) */
          {
            let base = 16 * i; // * (0x21 + i); // $21: 記号と数字, $41: 英大文字と感嘆/疑問符
            let pattern_low = &chr_rom[base..base + 0x8]; // 0 ~ 7
            let pattern_high = &chr_rom[base + 0x8..base + 0x10]; // 8 ~ 15

            for y in 0..8 {
              for x in 0..8 {
                // fn is_put(v: u8, x: u8) -> bool { (v >> x) & 1 == 1 }
                let dx = ((7 - x) + i % 32 * 8) as u32 + WIDTH + 275;
                let dy = y as u32 + (i / 32) as u32 * 8 + 15;

                let put_low = (pattern_low[y] >> x) & 1 == 1;
                let put_high = (pattern_high[y] >> x) & 1 == 1;

                let color = if put_low || put_high {
                  if put_low && put_high {
                    Rgba([255; 4])
                  } else {
                    Rgba([127, 127, 127, 255])
                  }
                } else {
                  Rgba([0, 0, 10, 150])
                };

                debug_screen.put_pixel(dx, dy, color);
              }
            }
          }

          debug_texture
            .update(&mut debug_texture_context, &debug_screen)
            .unwrap();

          debug_texture_context.encoder.flush(d);
          image(&debug_texture, c.transform.scale(2.0, 2.0), g);
        }
      });
    }
  }
}

// テストクン
#[test]
fn cpu_register() {
  let module = cpu::Cpu::new();
  assert_eq!(module.a, 0);
  assert_eq!(module.x, 0);
  assert_eq!(module.y, 0);
  assert_eq!(module.pc, 0);
  assert_eq!(module.sp, 0xfe);
  assert_eq!(module.p, 0x20);
}

#[test]
fn load_cassette() {
  let path = "./roms/sample1.nes".to_string();
  let mut machine = machine::Machine::new();
  let result = system::load_cassette(&mut machine, path, false);

  match result {
    Ok(_) => (),
    _ => panic!("カスのカセット、カスット"), // 了解！
  }
}

#[test]
fn check_rom_data() {
  let prg_data = [0x78, 0xa2, 0xff, 0x9a, 0xa9, 0x00, 0x8d, 0x00, 0x20, 0x8d]; // from 0
  let chr_data = [0x1c, 0x3e, 0x3e, 0x3e, 0x1c, 0x1c, 0x1c, 0x1c, 0x18, 0x3c]; // from 528
  let path = "./roms/sample1.nes".to_string();
  let mut machine = machine::Machine::new();
  let result = system::load_cassette(&mut machine, path, false);

  for id in 0..10 {
    match &result {
      Ok((prg_rom, chr_rom)) => {
        assert_eq!(prg_rom[id], prg_data[id]);
        assert_eq!(chr_rom[id + 528], chr_data[id]);
      }

      _ => {}
    }
  }
}

#[test]
fn transfer_bytes() {
  let path = "./roms/sample1.nes".to_string();
  let mut machine = machine::Machine::new();
  system::load_cassette(&mut machine, path, false);
  assert_eq!(machine.prg_bytes, 32768); // PRG_ROM size of Hello World
  assert_eq!(machine.chr_bytes, 8192); // CHR_ROM size of Hello World
}

/*
#[test]
fn stack_and_pop() {
  let addr = 0x1fe;
  let mut cpu = cpu::Cpu::new();
  let mut machine = machine::Machine::new();

  assert_eq!(cpu.pop_stack(&mut machine), 0); // sp がインクリメントされる
  cpu.push_stack(&mut machine, 0x7b);
  assert_eq!(cpu.pop_stack(&mut machine), 0x7b); // インクリメントされた分ずらす
}
*/
