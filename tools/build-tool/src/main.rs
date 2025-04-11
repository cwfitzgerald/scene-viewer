use std::{
    process::Command,
    sync::LazyLock,
    time::{Duration, SystemTime},
};

use indicatif::{ProgressBar, ProgressStyle};

const SHADER_LIST: &[ShaderDecl] = &[
    ShaderDecl {
        path: "triangle.slang",
        entry_point: "vert_main",
        output: "triangle.vert.spv",
    },
    ShaderDecl {
        path: "triangle.slang",
        entry_point: "frag_main",
        output: "triangle.frag.spv",
    },
];

const SHADER_SOURCE_DIRECTORY: &str = "crates/render-common/shaders";
const VULKAN_BUILT_SHADER_DIRECTORY: &str = "crates/render-vulkan/shaders";

static EXE_MODIFICATION_TIME: LazyLock<SystemTime> = LazyLock::new(|| {
    std::env::current_exe()
        .unwrap()
        .metadata()
        .unwrap()
        .modified()
        .unwrap()
});

struct ShaderDecl {
    path: &'static str,
    entry_point: &'static str,
    output: &'static str,
}

impl ShaderDecl {
    fn task(&self) -> Option<Task> {
        let input_path = format!("{SHADER_SOURCE_DIRECTORY}/{}", self.path);
        let output_path = format!("{VULKAN_BUILT_SHADER_DIRECTORY}/{}", self.output);

        let command = String::from("slangc");
        let args = vec![
            input_path.clone(),
            String::from("-entry"),
            String::from(self.entry_point),
            String::from("-capability"),
            String::from("spirv_1_4+SPV_EXT_physical_storage_buffer+spvSparseResidency"),
            String::from("-o"),
            output_path.clone(),
        ];

        Task::new(command, args, input_path, output_path)
    }
}

struct Task {
    command: String,
    args: Vec<String>,
}

impl Task {
    fn new(
        command: String,
        args: Vec<String>,
        input_file: String,
        output_file: String,
    ) -> Option<Self> {
        let input_modified = std::fs::metadata(&input_file)
            .unwrap_or_else(|_| panic!("Failed to get metadata for input file: {}", input_file))
            .modified()
            .unwrap_or_else(|_| {
                panic!("Failed to get modified time for input file: {}", input_file)
            });

        let output_metadata = std::fs::metadata(&output_file).ok();
        let Some(output_metadata) = output_metadata else {
            // If the output file doesn't exist, we need to run the task
            return Some(Task { command, args });
        };

        let output_modified = output_metadata.modified().unwrap_or_else(|_| {
            panic!(
                "Failed to get modified time for output file: {}",
                output_file
            )
        });

        if input_modified > output_modified || *EXE_MODIFICATION_TIME > output_modified {
            Some(Task { command, args })
        } else {
            None
        }
    }

    fn run(self) {
        let command: std::process::Output = Command::new(&self.command)
            .args(&self.args)
            .output()
            .expect("Failed to execute command");

        if !command.status.success() {
            eprintln!(
                "Error: Command `{} {}` failed with status code {:?}:\nSTDERR:\n{}\nSTDOUT:\n{}",
                self.command,
                self.args.join(" "),
                command.status.code(),
                String::from_utf8_lossy(&command.stderr),
                String::from_utf8_lossy(&command.stdout),
            );
            std::process::exit(1);
        }
    }
}

fn main() {
    let remaining_args = std::env::args().skip(1).collect::<Vec<_>>();

    let mut shader_tasks = Vec::new();
    for shader in SHADER_LIST {
        shader_tasks.extend(shader.task());
    }

    let shader_progress = ProgressBar::new(shader_tasks.len() as u64);
    shader_progress.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg} [{bar}] {pos}/{len}")
            .unwrap()
            .progress_chars("##-"),
    );
    shader_progress.set_message("Compiling shaders");
    shader_progress.enable_steady_tick(Duration::from_millis(100));

    rayon::scope(|s| {
        for task in shader_tasks {
            s.spawn(|_| {
                task.run();
                shader_progress.inc(1);
            });
        }
    });

    shader_progress.finish();

    println!("Building scene-viewer...");

    let status = Command::new("cargo")
        .arg("run")
        .arg("--package")
        .arg("scene-viewer")
        .args(remaining_args)
        .status()
        .expect("Failed to run cargo");

    if !status.success() {
        std::process::exit(1);
    }
}
