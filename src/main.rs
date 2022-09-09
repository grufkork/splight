#![windows_subsystem = "windows"]

use eframe::{egui::{TextBuffer, Visuals, Color32, Frame, Rect, Pos2, Vec2, Sense, RichText, TextFormat, FontFamily, FontId, Label}, epaint::{Rounding, Stroke}};
use inputbot::{KeybdKey, BlockInput};

use eframe;
use eframe::egui;

use std::{
    ops::RangeInclusive, 
    sync::{
        Arc, 
        Mutex
    },
    thread, 
    fs::{
        read_to_string, 
        DirEntry, 
        read_dir
    }, error::Error
};

use std::process::Command;

#[derive(Debug, Clone)]
struct Path{
    alias: String,
    path: String
}

#[derive(Debug)]
struct Program{
    char: String,
    flags: Vec<String>,
    program: String,
    display_name: String
}

fn load() -> Result<(Vec<Path>, Vec<Program>), Box<dyn Error>>{
    let mut paths: Vec<Path> = vec![]; 
    for line in read_to_string("paths.txt")?.split("\r\n"){
        let split: Vec<&str> = line.split(" ").collect();
        paths.push({
            Path{
                alias: split[0].to_string(),
                path: split[1..].join(" ")
            }
        });
    }

    let mut programs: Vec<Program> = vec![];
    for program in read_to_string("commands.txt").unwrap().split("\r\n\r\n"){
        let mut lines = program.split("\r\n");
        let mut params = lines.next().unwrap().split(" ");
        programs.push(Program { 
            display_name: params.next().unwrap().to_string(),
            char: params.next().unwrap().to_string(), 
            program: params.collect::<Vec<&str>>().join(" "), 
            flags: lines.map(|l|l.to_string()).collect(),
        });
    }

    Ok((paths, programs))
}

fn main() -> Result<(), ()>{
    println!("Starting...");

    let mut error: Option<String> = None;
    
    let loaded = load();
    let (paths, programs) = match loaded{
        Ok(res) => {
            res
        },
        Err(e) => {
            error = Some(e.to_string());
            (vec![], vec![])
        }
    };
    

    println!("{} paths, {} programs", paths.len(), programs.len());




    let opts = eframe::NativeOptions{
        decorated: false,
        always_on_top: true,
        transparent: true,
        ..Default::default()
    };


    eframe::run_native(
        "splight",
        opts,
        Box::new(|ctx| {
            let running = Arc::new(Mutex::new(true));

            let runclone = running.clone();
            let ctxclone = ctx.egui_ctx.clone();

            thread::spawn(move ||{
                KeybdKey::EnterKey.blockable_bind(move ||{
                    if KeybdKey::RShiftKey.is_pressed(){
                        *runclone.lock().unwrap() = true;
                        ctxclone.request_repaint();
                        BlockInput::Block
                    }else{
                        BlockInput::DontBlock
                    }
                });
                inputbot::handle_input_events();
            });

            Box::new( App{
                cmd: format!(" {} programs, {} paths ", programs.len(), paths.len()), 
                paths,
                programs,
                pathpieces: vec![],
                pathcache: vec![],
                matches: vec![],
                running,
                error
            })
        }
        )
    );


    

    Ok(())
}


struct App {
    cmd: String,

    paths: Vec<Path>,
    programs: Vec<Program>,

    pathpieces: Vec<String>,

    pathcache: Vec<DirEntry>,

    matches: Vec<usize>,
    running: Arc<Mutex<bool>>,
    error: Option<String>
}

impl App{
    fn reset(&mut self){
        self.cmd = String::new();
        self.matches = vec![];
        self.pathpieces = vec![];
    }

    fn get_path(&self) -> String{
        format!("{}{}", 
            if self.pathpieces.is_empty(){
                String::new()
            }else{
                format!("{}\\", self.pathpieces.join("\\"))
            }, 
            if self.matches.is_empty(){
                String::new()
            }else{
                if self.pathpieces.len() > 0{
                    self.pathcache[self.matches[0]].file_name().to_str().unwrap().to_string()
                }else{
                    self.paths[self.matches[0]].path.clone()
                }
            }
        ).split("\\\\").collect::<Vec<&str>>().join("\\")
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> egui::Rgba {
        egui::Rgba::from_white_alpha(0.)
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.error.is_some(){
            frame.set_window_pos(Pos2::new(100., 100.));
            frame.set_window_size(Vec2::new(300., 100.));
            egui::CentralPanel::default()
            .frame(Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui|{

                ui.add(Label::new(RichText::new(format!("Error: {}", self.error.as_ref().unwrap()))
                    .color(Color32::RED)));
            });
                
            return;
        }
        if *self.running.lock().unwrap(){
            let mut size = Vec2::splat(100.);
            frame.set_visible(true);
            frame.set_decorations(false);
            frame.set_window_pos(Pos2::new(100., 50.));

            egui::SidePanel::left("main")
            //.resizable(false)
            //.title_bar(false)
            .frame(Frame::none()
                .fill(Color32::BLACK)
                //.rounding(Rounding::same(10.))
                //.stroke(eframe::epaint::Stroke::new(3., Color32::WHITE))
                //.inner_margin(Margin::same(10.))
                //.outer_margin(Margin::same(10.))
            )
            .width_range(RangeInclusive::new(0., 9999.))
            .default_width(800.)
            .resizable(false)
            .show(ctx, |ui| {
                ctx.set_visuals(Visuals{
                    override_text_color: Some(Color32::from_gray(200)),
                    ..Default::default()
                });

                ui.allocate_rect(Rect::from_center_size(Pos2::new(0.0, -100.0), Vec2::new(0., -100.)), Sense::click());

                let input = ui.text_edit_singleline(&mut self.cmd);
                input.request_focus();

                if input.changed() && self.cmd.len() >= 1{

                    let split = self.cmd.get(1..).unwrap().split(" ").collect::<Vec<&str>>();

                    // Split should be one longer than the pathpieces, as the last split part is being edited
                    if split.len() != self.pathpieces.len() + 1{
                        if split.len() >= self.pathpieces.len() + 2
                        {
                            if self.matches.is_empty(){
                                self.reset();
                                return;
                            }else{
                                if split.len() == 2{
                                    self.pathpieces.push(self.paths[self.matches[0]].path.clone());
                                }else{
                                    self.pathpieces.push(self.pathcache[self.matches[0]].file_name().to_str().unwrap().to_string());
                                }
                            }
                        }else if split.len() <= self.pathpieces.len(){
                            self.pathpieces.pop();
                        }

                        if split.len() > 1{
                            let dir = read_dir(self.pathpieces.join("\\"));
                            self.pathcache = dir.unwrap().filter_map(|f| 
                                if let Ok(res) = f && let Ok(t) = res.file_type() && t.is_dir() {
                                    Some(res)
                                }else{
                                    None
                                }
                            ).collect();
                        }
                    }


                    self.matches = vec![];
                    if split.len() > 1 { // Search directory
                        for dir in self.pathcache.iter().enumerate(){
                            if dir.1.file_name().to_str().unwrap().to_ascii_lowercase().contains(split.last().unwrap()){
                                self.matches.push(dir.0);
                            }
                        }
                    }else{ // Search bookmarks
                        for path in self.paths.iter().enumerate(){
                            if path.1.alias.contains(split[0]){
                                self.matches.push(path.0);
                            }
                        }
                    }
                }

                if input.ctx.input().key_pressed(eframe::egui::Key::Enter){
                    input.request_focus();
                    if self.cmd.len() > 0{
                        let command = self.cmd.get(0..1).unwrap();
                        let path = self.get_path();
                        if command.as_str() == "q"{
                            frame.close();
                        }else{
                            for program in &self.programs{
                                if command == program.char{
                                    Command::new(&program.program)
                                        .args(program.flags.iter().map(|f| f.replacen("PATH", &path, 1)))
                                        .spawn().unwrap();
                                    break;
                                }
                            }
                        }
                    }

                    *self.running.lock().unwrap() = false;
                    self.reset();
                }
                    
                ui.allocate_rect(Rect::from_two_pos(Pos2::new(0.0, 0.0), Pos2::new(0., 0.)), Sense::click());
                ui.vertical(|ui| {
                    ui.allocate_ui_at_rect(Rect::from_two_pos(Pos2::new(10., 10.), Pos2::new(9999., 9999.)), |ui|{
                        let fattext = TextFormat{
                            font_id: FontId::new(25., FontFamily::Proportional),
                            color: Color32::from_gray(200),
                            ..Default::default()
                        };
                        
                        let mut cmdline = egui::text::LayoutJob::default();

                        cmdline.append(">", 0.0, fattext.clone());

                        if self.cmd.len() > 0{
                            cmdline.append(self.cmd.get(0..1).unwrap(), 10.0, TextFormat{
                                color: Color32::RED,
                                ..fattext.clone()
                            });
                            if self.cmd.len() > 1{
                                cmdline.append(self.cmd.get(1..).unwrap(), 10.0, fattext);
                            }
                        }

                        ui.add(Label::new(cmdline));

                        ui.label(RichText::new(format!("{} | {}", 
                            {
                                let command = self.cmd.get(0..1).unwrap_or("");
                                match self.programs.iter().filter(|x| x.char==command).next(){
                                    Some(prg) => prg.display_name.as_str(),
                                    None => {
                                        match command{
                                            "q" => "QUIT",
                                            "" => "---",
                                            _ => "NOT FOUND"
                                        }
                                    }
                                }
                            },
                            self.get_path()
                        )).size(11.).monospace());

                        //let sep = ui.separator();

                        let mut first = true;
                        for m in &self.matches{
                            ui.add(Label::new(
                                {
                                    let mut text = RichText::new(
                                        if self.pathpieces.len() == 0{
                                            format!("{} - {}", self.paths[*m].alias, self.paths[*m].path)
                                        }else{
                                            self.pathcache[*m].file_name().to_str().unwrap().to_string()
                                        }
                                    );

                                    if first{
                                        text = text.strong().background_color(Color32::from_gray(50)).size(15.);
                                    }else{
                                        //text = text.background_color(Color32::Bl)
                                    }
                                    
                                    text
                                }
                            ).wrap(false));
                            first = false;
                        }
                        ui.shrink_height_to_current();
                        
                    });
                    ui.shrink_height_to_current();
                    let mut r = ui.min_rect();
                    //r.min -= Vec2::splat(5.);
                    r.max += Vec2::splat(10.);
                    //ui.painter().rect_stroke(r.shrink(1.), Rounding::same(1.), Stroke::new(0.1, Color32::WHITE));
                    size = r.max.to_vec2();
                    size.x += 5.;
                });
                ui.shrink_height_to_current();
            });
            frame.set_window_size(size);
        }else {
            frame.set_visible(false);
        }
    }
    
}