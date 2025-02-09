use std::{process::Command, time::Instant};

use anyhow::{anyhow, Context, Result};
use directories_next::ProjectDirs;
use serde::Deserialize;
use tinybit::{widgets::Text, Color};
use tinybit::{ScreenPos, Viewport};

use crate::MeterTheme;

pub fn load() -> Result<Conf> {
    let config_file = ProjectDirs::from("github", "the-gorg", "wonky")
        .context("project directory not found")?
        .config_dir()
        .join("config.toml");
    let buf = std::fs::read(&config_file).with_context(|| {
        anyhow!("no config file found at: {}", config_file.display())
    })?;

    toml::from_slice(&buf).map_err(Into::into)
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bloatie: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Widget {
    Meter(Meter),
    Indicator(Indicator),
    Seperator(Seperator),
}

#[derive(Debug, Deserialize)]
pub struct Conf {
    pub widgets: Vec<Widget>,
    pub settings: Settings,
}

#[derive(Debug, Deserialize)]
pub struct Seperator {
    pub title: Option<String>,
    pub right: bool,
    pub bottom: bool,
}

impl Seperator {}

#[derive(Debug, Deserialize)]
pub struct Indicator {
    title: Option<String>,
    command: String,
    frequency: u64,

    pub right: bool,
    pub bottom: bool,

    #[serde(skip_deserializing)]
    value: bool,
    #[serde(skip_deserializing)]
    reading: String,
    #[serde(skip_deserializing)]
    timer: Option<Instant>,
}

impl Indicator {
    pub fn update(&mut self) -> Result<()> {
        if self
            .timer
            .map(|t| t.elapsed().as_secs() > self.frequency)
            .unwrap_or(true)
        {
            self.timer = Some(Instant::now());

            if let Some(mut cmd) = construct_command(&self.command) {
                self.value = cmd.get_stdout().parse()?;
            }
        }

        Ok(())
    }

    pub fn init(&mut self) -> Result<()> {
        if let Some(output) =
            construct_command(&self.command).map(|mut cmd| cmd.get_stdout())
        {
            let mut split = output.split(' ');

            if let Some(value) = split.next() {
                self.value = value.parse()?;
                self.reading = split.collect();
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Meter {
    pub title: String,
    pub unit: String,
    pub prefix: Option<String>,

    max_command: String,
    value_command: String,
    frequency: u64,

    pub right: bool,
    pub bottom: bool,

    pub meter: bool,
    pub reading: bool,

    pub theme: usize,

    #[serde(skip_deserializing)]
    pub max_value: u64,
    #[serde(skip_deserializing)]
    pub current_value: u64,

    #[serde(skip_deserializing)]
    max_cmd: Option<Command>,
    #[serde(skip_deserializing)]
    value_cmd: Option<Command>,
    #[serde(skip_deserializing)]
    timer: Option<Instant>,
}

pub trait CommandExt {
    fn get_stdout(&mut self) -> String;
}

impl CommandExt for Command {
    fn get_stdout(&mut self) -> String {
        let output = self.output().expect("oops").stdout;

        std::str::from_utf8(&output)
            .expect("berp")
            .trim()
            .to_string()
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            title: "RAM".to_string(),
            unit: "mb".to_string(),
            max_value: 0,
            current_value: 0,
            max_command: "echo 16014".to_string(),
            value_command: "memcheck".to_string(),
            frequency: 1,
            timer: None,
            value_cmd: construct_command("memcheck"),
            max_cmd: construct_command("echo 16000"),
            prefix: None,
            theme: 1,
            right: true,
            bottom: false,
            meter: true,
            reading: true,
        }
    }
}

impl Meter {
    pub fn update(&mut self) -> Result<()> {
        if self
            .timer
            .map(|t| t.elapsed().as_secs() > self.frequency)
            .unwrap_or(true)
        {
            self.timer = Some(Instant::now());

            if let Some(mut cmd) = construct_command(&self.value_command) {
                self.current_value = cmd.get_stdout().parse()?;
            }
        }

        Ok(())
    }

    pub fn init(&mut self) -> Result<()> {
        if let Some(mut cmd) = construct_command(&self.max_command) {
            self.max_value = cmd.get_stdout().parse()?;
        }

        Ok(())
    }

    pub fn new() -> Self {
        Self::default()
    }
}

//-------------------------------------------------------------------------------------
// Drawing
//-------------------------------------------------------------------------------------
impl Meter {
    pub fn update_and_draw(
        &mut self,
        viewport: &mut Viewport,
        pos: &mut ScreenPos,
        theme: &MeterTheme,
    ) -> Result<()> {
        self.update()?;

        viewport.draw_widget(
            &Text::new(self.title.clone(), fg_color(), None),
            ScreenPos::new(pos.x, pos.y),
        );

        if self.reading {
            let value_reading = Text::new(
                format!(
                    "{}/{}{}",
                    self.current_value, self.max_value, self.unit
                ),
                fg_color(),
                None,
            );

            viewport.draw_widget(
                &value_reading,
                ScreenPos::new(
                    // TODO: why 2?!?
                    pos.x
                        + (viewport.size.width / 2
                            - 2
                            - value_reading.0.len() as u16),
                    pos.y.saturating_sub(1),
                ),
            );
        }
        if self.title != "" {
            viewport.draw_widget(
                &Text::new(self.title.clone(), fg_color(), None),
                ScreenPos::new(pos.x, pos.y.saturating_sub(1)),
            );
        }

        theme.draw(
            viewport,
            self,
            (self.current_value as f32, self.max_value as f32),
            ScreenPos::new(pos.x, pos.y),
        );

        Ok(())
    }
}

impl Seperator {
    //
    pub fn draw(
        &mut self,
        viewport: &mut Viewport,
        pos: &mut ScreenPos,
    ) -> Result<()> {
        if let Some(t) = &self.title {
            viewport.draw_widget(
                &Text::new(t, fg_color(), None),
                ScreenPos::new(pos.x, pos.y),
            );
        }

        Ok(())
    }
}

impl Indicator {
    //
    pub fn draw_and_update(
        &mut self,
        viewport: &mut Viewport,
        pos: &mut ScreenPos,
    ) -> Result<()> {
        self.update()?;
        let colors = match self.value {
            true => (Some(Color::Black), fg_color()),
            false => (Some(Color::Black), bg_color()),
        };

        viewport.draw_widget(
            &Text::new(
                " ".repeat((viewport.size.width / 2 - 2) as usize),
                None,
                colors.1,
            ),
            *pos,
        );

        if let Some(t) = &self.title {
            viewport.draw_widget(
                &Text::new(t, colors.0, colors.1),
                ScreenPos::new(pos.x, pos.y),
            );
        }

        Ok(())
    }
}

//-------------------------------------------------------------------------------------
// Common
//-------------------------------------------------------------------------------------

fn construct_command(command: &str) -> Option<Command> {
    let mut split = command.split_whitespace();
    let cmd = split.next()?;

    let mut command = Command::new(cmd);
    command.args(split);

    Some(command)
}

#[allow(dead_code, clippy::unnecessary_wraps)]
fn fg_color() -> Option<Color> {
    Some(Color::Green)
}

#[allow(dead_code, clippy::unnecessary_wraps)]
fn bg_color() -> Option<Color> {
    Some(Color::DarkGreen)
}
