#![windows_subsystem = "windows"]

use eframe::egui;
use egui_plot::{Line, Legend, Plot};
use sysinfo::{System, Pid, RefreshKind, ProcessRefreshKind, CpuRefreshKind, MemoryRefreshKind, Disks};
use serde::{Serialize, Deserialize};
use chrono::Local;
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Clone)]
struct ProcessInfo {
    name: String,
    pid: u32,
    cpu_usage: f32,
    memory_usage: u64,
    status: String,
    user: Option<String>,
    command_line: Option<String>,
}

// Structure for storing disk information
#[derive(Clone)]
struct DiskInfo {
    name: String,
    total_space: u64,
    available_space: u64,
    used_space: u64,
    usage_percent: f32,
    disk_type: String,
    file_system: String,
}

// Structure for storing column widths
struct ColumnWidths {
    name: f32,
    cpu: f32,
    memory: f32,
    status: f32,
    user: f32,
}

struct ResourceMonitor {
    system: System,
    processes: Vec<ProcessInfo>,
    disks: Vec<DiskInfo>,
    process_indices: Vec<usize>,
    sort_column: SortColumn,
    sort_descending: bool,
    selected_pid: Option<u32>,
    update_interval: f32,
    last_update: std::time::Instant,
    history: VecDeque<(f64, f64)>,
    max_history_points: usize,
    show_system_info: bool,
    show_disk_info: bool,
    process_filter: String,
    show_charts: bool,
    row_height: f32,
    hovered_row: Option<usize>,
    energy_saving_mode: bool,
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, PartialEq)]
enum SortColumn {
    #[default]
    Name,
    Cpu,
    Memory,
    Status,
}

impl ResourceMonitor {
    fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        
        // Update CPU information
        system.refresh_cpu();
        
        let mut disks_info = Vec::new();
        
        // Get disk information
        let disks = Disks::new_with_refreshed_list(); 
        for disk in disks.list() {
            let total_space = disk.total_space();
            let available_space = disk.available_space();
            let used_space = total_space.saturating_sub(available_space);
            let usage_percent = if total_space > 0 {
                (used_space as f64 / total_space as f64 * 100.0) as f32
            } else {
                0.0
            };
            
            // Get disk type
            let disk_type_str = match disk.kind() {
                sysinfo::DiskKind::SSD => "SSD",
                sysinfo::DiskKind::HDD => "HDD",
                _ => "Unknown",
            }.to_string();
            
            // Get file system
            let file_system = disk.file_system().to_string_lossy().into_owned();
             
            disks_info.push(DiskInfo {
                name: disk.name().to_string_lossy().into_owned(),
                total_space,
                available_space,
                used_space,
                usage_percent,
                disk_type: disk_type_str,
                file_system,
            });
        }
        
        Self {
            system,
            processes: Vec::new(),
            disks: disks_info,
            process_indices: Vec::new(),
            sort_column: SortColumn::default(),
            sort_descending: false,
            selected_pid: None,
            update_interval: 1.0,
            last_update: std::time::Instant::now(),
            history: VecDeque::with_capacity(100),
            max_history_points: 100,
            show_system_info: true,
            show_disk_info: true,
            process_filter: String::new(),
            show_charts: true,
            row_height: 25.0,
            hovered_row: None,
            energy_saving_mode: false,
        }
    }

    fn update(&mut self, ctx: &egui::Context) {
    let is_focused = ctx.input(|i| i.viewport().focused).unwrap_or(false);
    
    let effective_interval = if is_focused {
        if self.energy_saving_mode {
            self.update_interval * 2.0
        } else {
            self.update_interval
        }
    } else {
        self.update_interval * 5.0
    };

    let now = std::time::Instant::now();
    let time_since_update = now.duration_since(self.last_update).as_secs_f32();
    
    if time_since_update < effective_interval {
        let time_until_next_update = effective_interval - time_since_update;
        ctx.request_repaint_after(std::time::Duration::from_secs_f32(time_until_next_update.max(0.1)));
        return;
    }
    
    self.last_update = now;
    
    self.system.refresh_cpu();
    self.system.refresh_processes();
    
    self.processes.clear();
    
    for (pid, process) in self.system.processes() {
        let name = if process.name().is_empty() {
            format!("PID: {}", pid.as_u32())
        } else {
            process.name().to_string()
        };
        
        if !self.process_filter.is_empty() {
            let filter_lower = self.process_filter.to_lowercase();
            if !name.to_lowercase().contains(&filter_lower) {
                continue;
            }
        }
        
        // Исправление: используем to_string_lossy() только для OsStr
        let command_line = process.cmd()
    .first()
    .map(|s| {
        let os_str: &std::ffi::OsStr = s.as_ref();
        os_str.to_string_lossy().to_string()
    });
        
        let process_info = ProcessInfo {
            name,
            pid: pid.as_u32(),
            cpu_usage: process.cpu_usage(),
            memory_usage: process.memory(),
            status: format!("{:?}", process.status()),
            user: process.user_id().map(|uid| uid.to_string()),
            command_line,
        };
        
        self.processes.push(process_info);
    }
    
    self.sort_process_indices();
    
    if time_since_update > self.update_interval * 2.0 {
        self.update_disk_info();
    }
    
    let total_cpu = self.system.global_cpu_info().cpu_usage();
    let used_memory = self.system.used_memory() as f64;
    
    if self.history.len() >= self.max_history_points {
        self.history.pop_front();
    }
    self.history.push_back((total_cpu as f64, used_memory / 1024.0 / 1024.0));
    
    ctx.request_repaint_after(std::time::Duration::from_secs_f32(effective_interval));
}

    fn update_disk_info(&mut self) {
        self.disks.clear();
        let disks = Disks::new_with_refreshed_list();
        
        for disk in disks.list() {
            let total_space = disk.total_space();
            let available_space = disk.available_space();
            let used_space = total_space.saturating_sub(available_space);
            let usage_percent = if total_space > 0 {
                (used_space as f64 / total_space as f64 * 100.0) as f32
            } else {
                0.0
            };
            
            let disk_type_str = match disk.kind() {
                sysinfo::DiskKind::SSD => "SSD",
                sysinfo::DiskKind::HDD => "HDD",
                _ => "Unknown",
            }.to_string();
            
            let file_system = disk.file_system().to_string_lossy().into_owned();
            
            self.disks.push(DiskInfo {
                name: disk.name().to_string_lossy().into_owned(),
                total_space,
                available_space,
                used_space,
                usage_percent,
                disk_type: disk_type_str,
                file_system,
            });
        }
    }

    fn sort_process_indices(&mut self) {
        if self.process_indices.len() != self.processes.len() {
            self.process_indices = (0..self.processes.len()).collect();
        }
        
        match self.sort_column {
            SortColumn::Name => {
                self.process_indices.sort_by(|&a, &b| {
                    let cmp = self.processes[a].name.cmp(&self.processes[b].name);
                    if self.sort_descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Cpu => {
                self.process_indices.sort_by(|&a, &b| {
                    let cmp = self.processes[a].cpu_usage
                        .partial_cmp(&self.processes[b].cpu_usage)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if self.sort_descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Memory => {
                self.process_indices.sort_by(|&a, &b| {
                    let cmp = self.processes[a].memory_usage.cmp(&self.processes[b].memory_usage);
                    if self.sort_descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Status => {
                self.process_indices.sort_by(|&a, &b| {
                    let cmp = self.processes[a].status.cmp(&self.processes[b].status);
                    if self.sort_descending { cmp.reverse() } else { cmp }
                });
            }
        }
    }

    fn kill_selected_process(&mut self) {
        if let Some(pid) = self.selected_pid {
            if let Some(process) = self.system.process(Pid::from_u32(pid)) {
                if process.kill() {
                    println!("Process {} killed", pid);
                }
            }
        }
    }

    fn calculate_column_widths(&self, available_width: f32) -> ColumnWidths {
        ColumnWidths {
            name: available_width * 0.35,
            cpu: available_width * 0.15,
            memory: available_width * 0.20,
            status: available_width * 0.15,
            user: available_width * 0.15,
        }
    }
}

impl eframe::App for ResourceMonitor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update(ctx);
        
        egui::TopBottomPanel::top("menu_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Update interval (sec): ");
                if ui.add(egui::Slider::new(&mut self.update_interval, 0.1..=5.0)).changed() {
                    self.last_update = std::time::Instant::now() - 
                        std::time::Duration::from_secs_f32(self.update_interval);
                }
                
                if ui.button("Update now").clicked() {
                    self.last_update = std::time::Instant::now() - 
                        std::time::Duration::from_secs_f32(self.update_interval);
                }
                
                ui.separator();
                
                ui.checkbox(&mut self.show_system_info, "System information");
                ui.checkbox(&mut self.show_disk_info, "Disk information");
                ui.checkbox(&mut self.show_charts, "Charts");
                ui.checkbox(&mut self.energy_saving_mode, "Energy saving");
                
                ui.label("Row height: ");
                ui.add(egui::Slider::new(&mut self.row_height, 20.0..=40.0));
                
                if ui.button("Export JSON").clicked() {
                    if let Ok(json) = serde_json::to_string_pretty(&self.processes) {
                        let filename = format!("processes_{}.json", 
                            Local::now().format("%Y%m%d_%H%M%S"));
                        if let Err(e) = std::fs::write(&filename, json) {
                            eprintln!("Error writing file: {}", e);
                        } else {
                            ui.add(egui::Label::new(egui::RichText::new(format!("Saved to {}", filename))
                                .color(egui::Color32::GREEN)));
                        }
                    }
                }
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_height = ui.available_height();
            
            let table_height = if self.show_charts {
                available_height * 0.7
            } else {
                available_height * 0.95
            };
            
            egui::TopBottomPanel::top("process_table_panel")
                .height_range(table_height..=table_height)
                .show_inside(ui, |ui| {
                    if self.show_system_info {
                        self.render_system_info(ui);
                        ui.separator();
                    }
                    
                    if self.show_disk_info {
                        self.render_disk_info(ui);
                        ui.separator();
                    }
                    
                    self.render_process_table(ui);
                });
            
            if self.show_charts {
                egui::TopBottomPanel::bottom("charts_panel")
                    .min_height(200.0)
                    .show_inside(ui, |ui| {
                        self.render_charts(ui);
                    });
            }
        });
        
        egui::SidePanel::right("details_panel")
            .min_width(250.0)
            .max_width(350.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_process_details(ui);
            });
    }
}

impl ResourceMonitor {
    fn render_system_info(&self, ui: &mut egui::Ui) {
        ui.heading("System Information");
        egui::Grid::new("system_grid")
            .num_columns(2)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("Total memory: ");
                ui.label(format!("{:.1} GB", 
                    self.system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0));
                ui.end_row();
                
                ui.label("Used memory: ");
                ui.label(format!("{:.1} GB", 
                    self.system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0));
                ui.end_row();
                
                ui.label("Free memory: ");
                ui.label(format!("{:.1} GB", 
                    self.system.free_memory() as f64 / 1024.0 / 1024.0 / 1024.0));
                ui.end_row();
                
                ui.label("Total CPU usage: ");
                ui.label(format!("{:.1}%", self.system.global_cpu_info().cpu_usage()));
                ui.end_row();
                
                ui.label("Process count: ");
                ui.label(self.processes.len().to_string());
                ui.end_row();
                
                ui.label("Uptime: ");
                ui.label(format!("{:.1} sec", System::uptime()));
                ui.end_row();
            });
    }

    fn render_disk_info(&self, ui: &mut egui::Ui) {
        ui.heading("Disk Information");
        
        for disk in &self.disks {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Disk {} ({}, {})", 
                        disk.name, disk.disk_type, disk.file_system));
                    
                    let usage_color = if disk.usage_percent > 90.0 {
                        egui::Color32::RED
                    } else if disk.usage_percent > 70.0 {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::GREEN
                    };
                    
                    ui.label(egui::RichText::new(format!("{:.1}%", disk.usage_percent))
                        .color(usage_color));
                });
                
                ui.add(egui::ProgressBar::new(disk.usage_percent / 100.0)
                    .desired_width(ui.available_width())
                    .fill(egui::Color32::from_rgb(0, 120, 215)));
                
                ui.horizontal(|ui| {
                    ui.label(format!("Used: {:.1} GB", 
                        disk.used_space as f64 / 1024.0 / 1024.0 / 1024.0));
                    ui.label(format!("Free: {:.1} GB", 
                        disk.available_space as f64 / 1024.0 / 1024.0 / 1024.0));
                    ui.label(format!("Total: {:.1} GB", 
                        disk.total_space as f64 / 1024.0 / 1024.0 / 1024.0));
                });
            });
            ui.separator();
        }
    }

    fn render_process_table(&mut self, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Processes");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.process_filter)
                .hint_text("Search processes...")
                .desired_width(150.0)
                .interactive(true));
        });
    });
    
    let column_widths = self.calculate_column_widths(ui.available_width());
    
    ui.horizontal(|ui| {
        let name_label = egui::RichText::new("Process Name").color(egui::Color32::from_gray(220));
        let name_response = ui.add_sized([column_widths.name, self.row_height], 
            egui::SelectableLabel::new(self.sort_column == SortColumn::Name, name_label)
        );
        if name_response.clicked() {
            if self.sort_column == SortColumn::Name {
                self.sort_descending = !self.sort_descending;
            } else {
                self.sort_column = SortColumn::Name;
                self.sort_descending = false;
            }
            self.sort_process_indices();
        }
        
        let cpu_label = egui::RichText::new("CPU %").color(egui::Color32::from_gray(220));
        let cpu_response = ui.add_sized([column_widths.cpu, self.row_height],
            egui::SelectableLabel::new(self.sort_column == SortColumn::Cpu, cpu_label)
        );
        if cpu_response.clicked() {
            if self.sort_column == SortColumn::Cpu {
                self.sort_descending = !self.sort_descending;
            } else {
                self.sort_column = SortColumn::Cpu;
                self.sort_descending = true;
            }
            self.sort_process_indices();
        }
        
        let memory_label = egui::RichText::new("Memory (MB)").color(egui::Color32::from_gray(220));
        let memory_response = ui.add_sized([column_widths.memory, self.row_height],
            egui::SelectableLabel::new(self.sort_column == SortColumn::Memory, memory_label)
        );
        if memory_response.clicked() {
            if self.sort_column == SortColumn::Memory {
                self.sort_descending = !self.sort_descending;
            } else {
                self.sort_column = SortColumn::Memory;
                self.sort_descending = true;
            }
            self.sort_process_indices();
        }
        
        let status_label = egui::RichText::new("Status").color(egui::Color32::from_gray(220));
        let status_response = ui.add_sized([column_widths.status, self.row_height],
            egui::SelectableLabel::new(self.sort_column == SortColumn::Status, status_label)
        );
        if status_response.clicked() {
            if self.sort_column == SortColumn::Status {
                self.sort_descending = !self.sort_descending;
            } else {
                self.sort_column = SortColumn::Status;
                self.sort_descending = false;
            }
            self.sort_process_indices();
        }
        
        let user_label = egui::RichText::new("User").color(egui::Color32::from_gray(220));
        ui.add_sized([column_widths.user, self.row_height], 
            egui::Label::new(user_label));
    });
    
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);
    
    let scroll_height = ui.available_height();
    
    egui::ScrollArea::vertical()
        .max_height(scroll_height)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let indices = self.process_indices.clone();
            let mut new_hovered_row = None;
            let mut select_pid = None;
            
            for &index in &indices {
                let process = &self.processes[index];
                let (is_hovered, is_clicked) = self.render_table_row(
                    ui, 
                    process, 
                    &column_widths, 
                    index,
                    self.row_height
                );
                
                if is_hovered {
                    new_hovered_row = Some(index);
                }
                
                if is_clicked {
                    select_pid = Some(process.pid);
                }
            }
            
            // Обработка выбора процесса после отрисовки
            if let Some(pid) = select_pid {
                self.selected_pid = Some(pid);
            }
            
            self.hovered_row = new_hovered_row;
        });
}

    fn render_table_row(
    &self,
    ui: &mut egui::Ui,
    process: &ProcessInfo,
    column_widths: &ColumnWidths,
    row_index: usize,
    row_height: f32,
) -> (bool, bool) {
    let bg_color = if row_index % 2 == 0 {
        egui::Color32::from_rgba_unmultiplied(30, 30, 30, 255)
    } else {
        egui::Color32::from_rgba_unmultiplied(40, 40, 40, 255)
    };

    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), row_height + 4.0),
        egui::Sense::click(),
    );
    let rect = response.rect;
    let is_hovered = response.hovered();
    let is_clicked = response.clicked();
    let is_selected = self.selected_pid.map(|pid| pid == process.pid).unwrap_or(false);

    // Фон
    painter.rect_filled(rect, 0.0, bg_color);

    // Рамка при наведении
    if is_hovered {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
        );
    }

    // Рамка выделения
    if is_selected {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 120, 215)),
        );
    }

    // Параметры текста
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let text_color = egui::Color32::from_gray(220);
    let x_offset = rect.min.x + 4.0;
    let y_center = rect.center().y;

    // Имя
    painter.text(
        egui::pos2(x_offset, y_center),
        egui::Align2::LEFT_CENTER,
        &process.name,
        font_id.clone(),
        text_color,
    );

    // CPU
    let cpu_x = x_offset + column_widths.name;
    let cpu_color = if process.cpu_usage > 50.0 {
        egui::Color32::from_rgb(255, 100, 100)
    } else if process.cpu_usage > 20.0 {
        egui::Color32::from_rgb(255, 200, 100)
    } else {
        text_color
    };
    painter.text(
        egui::pos2(cpu_x, y_center),
        egui::Align2::LEFT_CENTER,
        &format!("{:.1}%", process.cpu_usage),
        font_id.clone(),
        cpu_color,
    );

    // Memory
    let memory_x = cpu_x + column_widths.cpu;
    let memory_mb = process.memory_usage as f64 / 1024.0 / 1024.0;
    let memory_color = if memory_mb > 500.0 {
        egui::Color32::from_rgb(255, 100, 100)
    } else if memory_mb > 100.0 {
        egui::Color32::from_rgb(255, 200, 100)
    } else {
        text_color
    };
    painter.text(
        egui::pos2(memory_x, y_center),
        egui::Align2::LEFT_CENTER,
        &format!("{:.1} MB", memory_mb),
        font_id.clone(),
        memory_color,
    );

    // Status
    let status_x = memory_x + column_widths.memory;
    let status_color = if process.status.contains("Run") {
        egui::Color32::from_rgb(100, 200, 100)
    } else {
        egui::Color32::from_gray(150)
    };
    painter.text(
        egui::pos2(status_x, y_center),
        egui::Align2::LEFT_CENTER,
        &process.status,
        font_id.clone(),
        status_color,
    );

    // User
    let user_x = status_x + column_widths.status;
    painter.text(
        egui::pos2(user_x, y_center),
        egui::Align2::LEFT_CENTER,
        process.user.as_deref().unwrap_or("N/A"),
        font_id,
        text_color,
    );

    (is_hovered, is_clicked)
}

    fn render_charts(&self, ui: &mut egui::Ui) {
        ui.heading("Resource Usage Charts");
        
        ui.horizontal(|ui| {
            let cpu_plot = Plot::new("cpu_plot")
                .height(180.0)
                .width(ui.available_width() * 0.49)
                .legend(Legend::default())
                .label_formatter(|name, value| {
                    if name.is_empty() {
                        format!("CPU: {:.1}%", value.y)
                    } else {
                        format!("{name}: {:.1}%", value.y)
                    }
                });
            
            cpu_plot.show(ui, |plot_ui| {
                if !self.history.is_empty() {
                    let points: Vec<[f64; 2]> = self.history
                        .iter()
                        .enumerate()
                        .map(|(i, &(cpu, _))| [i as f64, cpu])
                        .collect();
                    
                    let line = Line::new(points)
                        .name("CPU %")
                        .color(egui::Color32::from_rgb(255, 100, 100));
                    plot_ui.line(line);
                }
            });
            
            let memory_plot = Plot::new("memory_plot")
                .height(180.0)
                .width(ui.available_width() * 0.49)
                .legend(Legend::default())
                .label_formatter(|name, value| {
                    if name.is_empty() {
                        format!("Memory: {:.1} MB", value.y)
                    } else {
                        format!("{name}: {:.1} MB", value.y)
                    }
                });
            
            memory_plot.show(ui, |plot_ui| {
                if !self.history.is_empty() {
                    let points: Vec<[f64; 2]> = self.history
                        .iter()
                        .enumerate()
                        .map(|(i, &(_, memory))| [i as f64, memory])
                        .collect();
                    
                    let line = Line::new(points)
                        .name("Memory MB")
                        .color(egui::Color32::from_rgb(100, 150, 255));
                    plot_ui.line(line);
                }
            });
        });
    }

    fn render_process_details(&mut self, ui: &mut egui::Ui) {
    ui.heading("Process Details");
    
    if let Some(pid) = self.selected_pid {
        // Находим индекс процесса и извлекаем все необходимые данные
        if let Some(index) = self.process_indices.iter().position(|&i| self.processes[i].pid == pid) {
            // Копируем данные, чтобы не держать ссылку на self.processes
            let (name, pid, cpu_usage, memory_usage, status, user, command_line) = {
                let p = &self.processes[self.process_indices[index]];
                (
                    p.name.clone(),
                    p.pid,
                    p.cpu_usage,
                    p.memory_usage,
                    p.status.clone(),
                    p.user.clone(),
                    p.command_line.clone(),
                )
            };
            
            egui::Grid::new("details_grid")
                .num_columns(2)
                .spacing([10.0, 5.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Name: ").color(egui::Color32::from_gray(220)));
                    ui.label(egui::RichText::new(&name).color(egui::Color32::WHITE));
                    ui.end_row();
                    
                    ui.label(egui::RichText::new("PID: ").color(egui::Color32::from_gray(220)));
                    ui.label(egui::RichText::new(pid.to_string()).color(egui::Color32::WHITE));
                    ui.end_row();
                     
                    ui.label(egui::RichText::new("CPU: ").color(egui::Color32::from_gray(220)));
                    ui.label(egui::RichText::new(format!("{:.1}% ", cpu_usage)).color(egui::Color32::WHITE));
                    ui.end_row();
                    
                    ui.label(egui::RichText::new("Memory: ").color(egui::Color32::from_gray(220)));
                    ui.label(egui::RichText::new(format!("{:.1} MB ", 
                        memory_usage as f64 / 1024.0 / 1024.0))
                        .color(egui::Color32::WHITE));
                    ui.end_row();
                     
                    ui.label(egui::RichText::new("Status: ").color(egui::Color32::from_gray(220)));
                    ui.label(egui::RichText::new(&status).color(egui::Color32::WHITE));
                    ui.end_row();
                    
                    ui.label(egui::RichText::new("User: ").color(egui::Color32::from_gray(220)));
                    ui.label(egui::RichText::new(user.as_deref().unwrap_or("N/A")).color(egui::Color32::WHITE));
                    ui.end_row();
                    
                    if let Some(cmd) = &command_line {
                        ui.label(egui::RichText::new("Command: ").color(egui::Color32::from_gray(220)));
                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            ui.label(egui::RichText::new(cmd).color(egui::Color32::WHITE));
                        });
                        ui.end_row();
                    }
                });
            
            ui.separator();
            
            ui.horizontal(|ui| {
                if ui.button("Terminate Process").clicked() {
                    self.kill_selected_process();
                }
            });
            
            ui.separator();
            
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("CPU Usage: ").color(egui::Color32::from_gray(220)));
                ui.add(egui::ProgressBar::new(cpu_usage / 100.0)
                    .text(format!("{:.1}% ", cpu_usage))
                    .desired_width(ui.available_width())
                    .fill(egui::Color32::from_rgb(0, 120, 215)));
                
                ui.label(egui::RichText::new("Memory Usage: ").color(egui::Color32::from_gray(220)));
                let memory_percent = (memory_usage as f64 / self.system.total_memory() as f64) * 100.0;
                ui.add(egui::ProgressBar::new(memory_percent as f32 / 100.0)
                    .text(format!("{:.1}% of total memory ", memory_percent))
                    .desired_width(ui.available_width())
                    .fill(egui::Color32::from_rgb(0, 120, 215)));
            });
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Select a process from the table ")
                .color(egui::Color32::from_gray(220)));
            ui.label(egui::RichText::new("to view details ")
                .color(egui::Color32::from_gray(220)));
            ui.add_space(20.0);
            ui.colored_label(egui::Color32::from_gray(150), "← Click on a row ");
        });
    }
}
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_resizable(true)
            .with_icon(load_icon()), // <-- Устанавливаем иконку
        ..Default::default()
    };
    
    eframe::run_native(
        "SysWatch",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Box::new(ResourceMonitor::new())
        }),
    )
}

// Функция для загрузки иконки из PNG
fn load_icon() -> egui::IconData {
    // Пытаемся загрузить PNG файл
    match image::open("assets/syswatch.png") {
        Ok(img) => {
            // Конвертируем в RGBA
            let img = img.into_rgba8();
            let (width, height) = img.dimensions();
            
            // Возвращаем данные иконки
            egui::IconData {
                rgba: img.into_raw(),
                width,
                height,
            }
        }
        Err(e) => {
            // Если не удалось загрузить иконку, выводим предупреждение
            // но не паникуем - программа запустится с иконкой по умолчанию
            eprintln!("Warning: Failed to load icon from assets/syswatch.png: {}", e);
            egui::IconData::default()
        }
    }
}