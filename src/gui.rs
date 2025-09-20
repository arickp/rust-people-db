use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Label, Orientation, TreeView, ListStore, TreeViewColumn, CellRendererText, SelectionMode, Dialog, Entry, FileChooserDialog, FileChooserAction, ResponseType, Box as GtkBox, Button as GtkButton, ComboBoxText, ScrolledWindow, TextView};
use gtk::gio::ApplicationFlags;
use log;
use std::cell::RefCell;
use std::rc::Rc;
use gettextrs::gettext;
use gtk::glib;
use regex;

mod person;
mod constants;
use person::Person;
use constants::{APP_ID, APP_NAME, GUI_TABLE_HEADER_COLUMNS, Sport};

// Global state to store loaded people
struct AppState {
    people: Vec<Person>,
    list_store: ListStore,
    tree_view: Rc<TreeView>,
    last_file: Option<std::path::PathBuf>,
}

impl AppState {
    fn new(list_store: ListStore, tree_view: &Rc<TreeView>) -> Self {
        Self {
            people: Vec::new(),
            list_store,
            tree_view: tree_view.clone(),
            last_file: None,
        }
    }

    fn format_sport_display(sport: &Sport) -> String {
        let name = match sport {
            Sport::Other(name) => name.to_string(),
            _ => sport.to_string(),
        };
        format!("{} {}", sport.emoji(), name)
    }

    fn update_display(&self) {
        // Clear existing data
        self.list_store.clear();
        
        if self.people.is_empty() {
            // Show prompt when no file is loaded
            self.list_store.set(
                &self.list_store.append(),
                &[
                    (0, &0u32),  // ID column must be u32
                    (1, &format!("<span style='italic'>{}</span>", gettext("No people loaded"))),
                    (2, &"".to_string()),
                    (3, &"".to_string()),
                    (4, &"".to_string()),
                ],
            );
            // Clear selection
            self.tree_view.selection().unselect_all();
            return;
        }
        
        // Add people data to the list store
        for person in &self.people {
            self.list_store.set(
                &self.list_store.append(),
                &[
                    (0, &person.id),
                    (1, &person.first_name),
                    (2, &person.last_name),
                    (3, &person.get_age().to_string()),
                    (4, &Self::format_sport_display(&person.favorite_sport)),
                ],
            );
        }
        // Clear selection after loading new data
        self.tree_view.selection().unselect_all();
    }
}

// Helper to show confirmation dialog
fn show_confirm_dialog(parent: &ApplicationWindow, message: &str, on_confirm: Box<dyn Fn() + 'static>) {
    let dialog = Dialog::with_buttons(
        Some(&gettext("Confirm")),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[(&gettext("Cancel"), ResponseType::Cancel), (&gettext("OK"), ResponseType::Ok)],
    );
    let content_area = dialog.content_area();
    let label = Label::builder().label(message).build();
    content_area.append(&label);
    
    dialog.connect_response(move |d, resp| {
        if resp == ResponseType::Ok {
            on_confirm();
        }
        d.close();
    });
    dialog.show();
}

// Helper to show the Add/Edit dialog
fn show_person_dialog(parent: &ApplicationWindow, person: Option<&Person>, on_save: Box<dyn Fn(Person) + 'static>) {
    let title = if person.is_some() { gettext("Edit Person") } else { gettext("Add Person") };
    let dialog = Dialog::with_buttons(
        Some(title.as_str()),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
    );
    let content_area = dialog.content_area();
    let vbox = GtkBox::builder().orientation(Orientation::Vertical).spacing(6).build();
    let first_name_entry = Entry::builder().placeholder_text(&gettext("First Name")).build();
    let last_name_entry = Entry::builder().placeholder_text(&gettext("Last Name")).build();
    
    let dob_entry = Entry::builder().placeholder_text(&gettext("Date of Birth (YYYY-MM-DD)")).build();
    let sport_combo = ComboBoxText::new();
    let custom_sport_entry = Entry::builder().placeholder_text(&gettext("Custom Sport")).build();
    
    // Populate sport dropdown with all known sports
    for sport in Sport::all_known_sports() {
        let display_text = format!("{} {}", sport.emoji(), sport);
        sport_combo.append_text(&display_text);
    }
    // Add "Other" option for custom sports
    sport_combo.append_text(&format!("{} {}", "", gettext("Other")));

    // Store the original person's ID for editing
    let original_id = person.map(|p| p.id);

    if let Some(p) = person {
        first_name_entry.set_text(&p.first_name);
        last_name_entry.set_text(&p.last_name);
        dob_entry.set_text(&p.date_of_birth.to_string());
        
        // Set the sport dropdown to the current sport
        let sport_display = format!("{} {}", p.favorite_sport.emoji(), p.favorite_sport);
        let known_sports = Sport::all_known_sports();
        let mut found_index = None;
        
        for (i, sport) in known_sports.iter().enumerate() {
            let display_text = format!("{} {}", sport.emoji(), sport);
            if display_text == sport_display {
                found_index = Some(i);
                break;
            }
        }
        
        if let Some(index) = found_index {
            sport_combo.set_active(Some(index as u32));
        } else {
            // If it's a custom sport, select "Other" and populate the custom entry
            sport_combo.set_active(Some(known_sports.len() as u32));
            if let Sport::Other(custom_name) = &p.favorite_sport {
                custom_sport_entry.set_text(custom_name);
            }
        }
    }

    vbox.append(&first_name_entry);
    vbox.append(&last_name_entry);
    vbox.append(&dob_entry);
    vbox.append(&sport_combo);
    vbox.append(&custom_sport_entry);
    
    // Initially hide the custom sport entry
    custom_sport_entry.set_visible(false);
    
    // Show/hide custom sport entry based on dropdown selection
    let custom_sport_entry_clone = custom_sport_entry.clone();
    sport_combo.connect_changed(move |combo| {
        let active_index = combo.active().unwrap_or(0);
        let total_items = Sport::all_known_sports().len() as u32 + 1; // +1 for "Other"
        let is_other_selected = active_index == total_items - 1;
        custom_sport_entry_clone.set_visible(is_other_selected);
    });
    content_area.append(&vbox);
    
    dialog.connect_response(move |d, resp| {
        if resp == ResponseType::Ok {
            // Validate date format with regex
            let date_text = dob_entry.text();
            let date_regex = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
            
            let date_of_birth = if date_regex.is_match(&date_text) {
                chrono::NaiveDate::parse_from_str(&date_text, "%Y-%m-%d").unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1900,1,1).unwrap())
            } else {
                chrono::NaiveDate::from_ymd_opt(1900,1,1).unwrap()
            };
            
            // Get sport from dropdown or custom entry
            let sport = if let Some(active_index) = sport_combo.active() {
                let known_sports = Sport::all_known_sports();
                let total_items = known_sports.len() as u32 + 1; // +1 for "Other"
                if active_index == total_items - 1 {
                    // "Other" is selected, use custom sport entry
                    Sport::from_string(&custom_sport_entry.text())
                } else {
                    // A known sport is selected, get it from the dropdown
                    if let Some(sport) = known_sports.get(active_index as usize) {
                        sport.clone()
                    } else {
                        Sport::Other("Unknown".to_string())
                    }
                }
            } else {
                Sport::Other("Unknown".to_string())
            };
            
            let person = if let Some(id) = original_id {
                // Editing: preserve the original ID
                Person::with_id(
                    id,
                    first_name_entry.text().to_string(),
                    last_name_entry.text().to_string(),
                    date_of_birth,
                    sport,
                )
            } else {
                // Adding: create new person with auto-generated ID
                Person::new(
                    first_name_entry.text().to_string(),
                    last_name_entry.text().to_string(),
                    date_of_birth,
                    sport,
                )
            };
            on_save(person);
        }
        d.close();
    });
    dialog.show();
}

fn main() {
    env_logger::init();
    
    // Initialize gettext with the domain name
    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain("people-db", "/tmp/locale").expect("Failed to bind text domain");
    gettextrs::textdomain("people-db").expect("Failed to set text domain");
    
    log::debug!("Current locale: {}", std::env::var("LANG").unwrap_or_default());
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_activate(build_ui);
    app.connect_open(open_file);
    app.run();
}

fn build_ui(app: &Application) {
    use gtk::glib;

    // Create menu bar with buttons
    let menu_bar = GtkBox::builder().orientation(Orientation::Horizontal).spacing(6).build();
    
    // File menu buttons
    let open_btn = GtkButton::builder().label(&gettext("Open")).build();
    let save_btn = GtkButton::builder().label(&gettext("Save")).build();
    let exit_btn = GtkButton::builder().label(&gettext("Exit")).build();
    
    // People menu buttons
    let add_btn = GtkButton::builder().label(&gettext("Add")).build();
    let edit_btn = GtkButton::builder().label(&gettext("Edit")).build();
    let delete_btn = GtkButton::builder().label(&gettext("Delete")).build();
    let print_btn = GtkButton::builder().label(&gettext("Print")).build();
    
    menu_bar.append(&open_btn);
    menu_bar.append(&save_btn);
    menu_bar.append(&exit_btn);
    menu_bar.append(&add_btn);
    menu_bar.append(&edit_btn);
    menu_bar.append(&delete_btn);
    menu_bar.append(&print_btn);

    // Create list store with column types
    let list_store = ListStore::new(
        &[
            u32::static_type(), 
            String::static_type(), 
            String::static_type(), 
            String::static_type(), 
            String::static_type(), 
        ]);
    
    // Create tree view
    let tree_view = Rc::new(TreeView::builder()
        .model(&list_store)
        .build());
    tree_view.selection().set_mode(SelectionMode::Single);
    
    // Create columns
    for (i, header) in GUI_TABLE_HEADER_COLUMNS.iter().enumerate() {
        let column = TreeViewColumn::new();
        let cell = CellRendererText::new();
        
        column.pack_start(&cell, true);
        column.add_attribute(&cell, "markup", i as i32);
        
        let title = gettext(*header);
        column.set_title(title.as_str());
        
        tree_view.append_column(&column);
    }

    // Layout
    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .build();
    vbox.append(&menu_bar);
    vbox.append(tree_view.as_ref());

    let window = ApplicationWindow::builder()
        .application(app)
        .title(&gettext(APP_NAME))
        .default_width(600)
        .default_height(400)
        .child(&vbox)
        .build();

    // Create app state
    let app_state = Rc::new(RefCell::new(AppState::new(list_store, &tree_view)));
    
    // Show initial prompt
    app_state.borrow().update_display();
    
    // Create action handlers
    let app_state_open = app_state.clone();
    let app_state_save = app_state.clone();
    let app_state_add = app_state.clone();
    let app_state_edit = app_state.clone();
    let app_state_delete = app_state.clone();
    let app_state_print = app_state.clone();
    let window_open = window.clone();
    let window_save = window.clone();
    let window_add = window.clone();
    let window_edit = window.clone();
    let window_delete = window.clone();
    let window_print = window.clone();
    
    // Connect button handlers
    open_btn.connect_clicked(glib::clone!(@weak window_open => move |_| {
        log::info!("Open button clicked");
        open_file_dialog(&window_open, app_state_open.clone());
    }));

    save_btn.connect_clicked(glib::clone!(@weak window_save, @weak app_state_save => move |_| {
        log::info!("Save button clicked");
        let mut state = app_state_save.borrow_mut();
        if let Some(ref file) = state.last_file {
            if let Err(e) = Person::write_to_csv(file, &state.people) {
                log::error!("Failed to save: {}", e);
            }
        } else {
            // Prompt for file
            let dialog = FileChooserDialog::builder()
                .title(&gettext("Save CSV File"))
                .transient_for(&window_save)
                .action(FileChooserAction::Save)
                .build();
            dialog.add_button("Cancel", ResponseType::Cancel);
            dialog.add_button("Save", ResponseType::Accept);
            dialog.connect_response(glib::clone!(@weak app_state_save => move |dialog, resp| {
                if resp == ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            let mut state = app_state_save.borrow_mut();
                            if let Err(e) = Person::write_to_csv(&path, &state.people) {
                                log::error!("Failed to save: {}", e);
                            } else {
                                state.last_file = Some(path);
                            }
                        }
                    }
                }
                dialog.close();
            }));
            dialog.show();
        }
    }));

    exit_btn.connect_clicked(move |_| {
        log::info!("Exit button clicked");
        std::process::exit(0);
    });

    add_btn.connect_clicked(glib::clone!(@weak window_add, @weak app_state_add => move |_| {
        log::info!("Add button clicked");
        show_person_dialog(&window_add, None, Box::new(glib::clone!(@weak app_state_add => move |person| {
            app_state_add.borrow_mut().people.push(person);
            app_state_add.borrow().update_display();
        })));
    }));

    edit_btn.connect_clicked(glib::clone!(@weak window_edit, @weak app_state_edit => move |_| {
        log::info!("Edit button clicked");
        let state = app_state_edit.borrow();
        if let Some((model, iter)) = state.tree_view.selection().selected() {
            let id_value: u32 = model.get::<u32>(&iter, 0);

            if let Some(idx) = state.people.iter().position(|p| p.id == id_value) {
                let person = state.people[idx].clone();
                log::info!("Editing person with ID {}", person.id);
                show_person_dialog(&window_edit, Some(&person), Box::new(glib::clone!(@weak app_state_edit => move |new_person| {
                    app_state_edit.borrow_mut().people[idx] = new_person;
                    app_state_edit.borrow().update_display();
                })));
            } else {
                log::warn!("No person found with ID {}", id_value);
            }
        } else {
            log::warn!("No selection found");
        }
    }));

    delete_btn.connect_clicked(glib::clone!(@weak window_delete, @weak app_state_delete => move |_| {
        log::info!("Delete button clicked");
        let state = app_state_delete.borrow();
        if let Some((model, iter)) = state.tree_view.selection().selected() {
            let id_value: u32 = model.get::<u32>(&iter, 0);  // column 0 is ID
            if let Some(idx) = state.people.iter().position(|p| p.id == id_value) {
                if let Some(person) = state.people.get(idx).cloned() {
                    let message = format!("{} {} {}?", gettext("Are you sure you want to delete"), person.first_name, person.last_name);
                    show_confirm_dialog(&window_delete, &message, Box::new(glib::clone!(@weak app_state_delete => move || {
                        let mut state = app_state_delete.borrow_mut();
                        if let Some((model, iter)) = state.tree_view.selection().selected() {
                            let id_value: u32 = model.get::<u32>(&iter, 0);  // column 0 is ID
                            if let Some(idx) = state.people.iter().position(|p| p.id == id_value) {
                                log::info!("Deleting person with ID {}", id_value);
                                state.people.remove(idx);
                                state.update_display();
                            } else {
                                log::warn!("No person found with ID {}", id_value);
                            }
                        }
                    })));
                } else {
                    log::warn!("Person not found at index {}", idx);
                }
            } else {
                log::warn!("No person found with ID {}", id_value);
            }
        } else {
            log::warn!("No selection found");
        }
    }));

    print_btn.connect_clicked(glib::clone!(@weak window_print, @weak app_state_print => move |_| {
        log::info!("Print button clicked");
        show_print_dialog(&window_print, app_state_print.clone());
    }));

    window.present();
}

// Helper to show print dialog
fn show_print_dialog(parent: &ApplicationWindow, app_state: Rc<RefCell<AppState>>) {
    let people = app_state.borrow().people.clone();
    
    if people.is_empty() {
        // Show message dialog for empty list
        let dialog = Dialog::with_buttons(
            Some(&gettext("Print")),
            Some(parent),
            gtk::DialogFlags::MODAL,
            &[(&gettext("OK"), ResponseType::Ok)],
        );
        let content_area = dialog.content_area();
        let label = Label::builder().label(&gettext("No people to print")).build();
        content_area.append(&label);
        
        dialog.connect_response(|d, _| d.close());
        dialog.show();
        return;
    }
    
    // Show people list in a dialog
    let dialog = Dialog::with_buttons(
        Some(&gettext("People List")),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[(&gettext("Close"), ResponseType::Ok)],
    );
    
    let content_area = dialog.content_area();
    
    // Create a text view to display the people
    let text_view = gtk::TextView::new();
    text_view.set_editable(false);
    text_view.set_wrap_mode(gtk::WrapMode::Word);
    
    // Create a scrolled window for the text view
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_child(Some(&text_view));
    scrolled_window.set_size_request(400, 300);
    
    // Format the people data
    let mut text = String::new();
    text.push_str(&format!("People Database Report\n"));
    text.push_str(&format!("Total people: {}\n\n", people.len()));
    
    for person in &people {
        text.push_str(&format!("ID: {}\n", person.id));
        text.push_str(&format!("Name: {} {}\n", person.first_name, person.last_name));
        text.push_str(&format!("Age: {}\n", person.get_age()));
        text.push_str(&format!("Favorite Sport: {} {}\n", person.get_favorite_sport_emoji(), person.favorite_sport));
        text.push_str("\n");
    }
    
    // Set the text in the text view
    let buffer = text_view.buffer();
    buffer.set_text(&text);
    
    content_area.append(&scrolled_window);
    
    dialog.connect_response(|d, _| d.close());
    dialog.show();
}


fn open_file_dialog(parent: &ApplicationWindow, app_state: Rc<RefCell<AppState>>) {
    let dialog = FileChooserDialog::builder()
        .title("Open CSV File")
        .transient_for(parent)
        .action(FileChooserAction::Open)
        .build();

    dialog.add_button("Cancel", ResponseType::Cancel);
    dialog.add_button("Open", ResponseType::Accept);

    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(file_path) = file.path() {
                    log::info!("Opening file: {:?}", file_path);
                    match Person::read_from_csv(&file_path) {
                        Ok(people) => {
                            log::info!("Loaded {} people", people.len());
                            app_state.borrow_mut().people = people;
                            app_state.borrow_mut().last_file = Some(file_path);
                            app_state.borrow().update_display();
                        }
                        Err(e) => {
                            log::error!("Failed to load people: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
        dialog.close();
    });

    dialog.show();
}

fn open_file(_app: &Application, files: &[gtk::gio::File], _hint: &str) {
    if let Some(file) = files.first() {
        if let Some(file_path) = gtk::gio::prelude::FileExt::path(file) {
            log::info!("Opening file: {:?}", file_path);
            match Person::read_from_csv(&file_path) {
                Ok(people) => {
                    log::info!("Loaded {} people", people.len());
                }
                Err(e) => {
                    log::error!("Failed to load people: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

