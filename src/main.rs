/// This is the sourcecode for the `osbuild-ks` command line utility. This program transforms
/// Kickstart files to osbuild manifest files.
///
/// The general approach used is to translate Kickstart syntax to osbuild stages and their
/// arguments. For the parts of Kickstart that use direct shell commands some debatable parsing is
/// used to convert things to stages that perform the same actions.
///
/// You should listen to Mötley Crüe - Kickstart My Heart while reading this file to put you in the
/// right mindset: https://www.youtube.com/watch?v=CmXWkMlKFkI
///
/// This file is licensed under the Apache 2.0 license and you can find its repository on
/// [GitHub](https://github.com/supakeen/osbuild-ks) which is also where any bugs and issues can be
/// filed. The specification and abilities of Kickstart files were found on this
/// [Fedora Documentation](https://docs.fedoraproject.org/en-US/fedora/latest/install-guide/appendixes/Kickstart_Syntax_Reference/)
/// page.
use std::path::Path;
use std::process::exit;

use clap;
use log::*;

mod kickstart {
    use std::fs;
    use std::io;
    use std::io::prelude::*;
    use std::path::{Path, PathBuf};
    use std::process::exit;

    use log::*;

    #[derive(Clone, Debug)]
    pub struct Kickstart {
        file: File,
        tree: Tree,
    }

    #[derive(Clone, Debug)]
    pub struct File {
        path: Box<PathBuf>,
        data: String,
    }

    #[derive(Clone, Debug)]
    pub struct Section {
        name: String,
        data: String,
        args: Vec<String>,
    }

    #[derive(Clone, Debug)]
    pub struct Tree {
        file: File,
        sections: Vec<Section>,
    }

    #[derive(Debug)]
    pub enum KickstartError {
        IO(io::Error),
        Parse,
    }

    impl From<io::Error> for KickstartError {
        fn from(err: io::Error) -> KickstartError {
            KickstartError::IO(err)
        }
    }

    impl Kickstart {
        pub fn from_path<'a>(src: &Path, inc: &Path) -> Result<Self, KickstartError> {
            let src = &src.canonicalize()?;
            let inc = &inc.canonicalize()?;

            info!(
                "Creating Kickstart from path '{}' with include path '{}'",
                src.display(),
                inc.display()
            );

            let file = File::from_path(src, inc)?;
            let tree = Tree::from_file(file.clone())?.parse(); // TODO: no clone

            Ok(Self {
                file: file,
                tree: tree,
            })
        }
    }

    impl File {
        pub fn from_path(src: &Path, inc: &Path) -> Result<Self, KickstartError> {
            let mut file = fs::File::open(src)?;
            let mut buffer = String::new();

            file.read_to_string(&mut buffer)?;

            let mut instance = Self {
                path: Box::new(src.canonicalize()?),
                data: buffer,
            };

            instance.clean();
            instance.resolve(&inc)?;

            Ok(instance)
        }

        /// Remove all comments from a kickstart file.
        fn clean(&mut self) -> Result<(), KickstartError> {
            let mut buf = String::new();

            for line in self.data.lines() {
                if !line.starts_with("#") {
                    buf = buf + line + "\n";
                }
            }

            self.data = buf;

            Ok(())
        }

        /// Resolve all includes in a kickstart file to flatten it into a single string.
        fn resolve(&mut self, inc: &Path) -> Result<(), KickstartError> {
            let mut data = String::new();

            for line in self.data.lines() {
                if line.starts_with("%include") {
                    // TODO: handle ksappend as well and check order
                    let parts: Vec<&str> = line.split_whitespace().collect();

                    if parts.len() != 2 {
                        eprintln!("ErroR!");
                        exit(1);
                    }

                    trace!(
                        "File.resolve: '{}' wants to include '{}'",
                        self.path.display(),
                        parts[1]
                    );

                    let path = Path::join(inc, Path::new(parts[1]));

                    if !path.exists() {
                        eprintln!("Error!");
                        exit(2);
                    }

                    let string = File::from_path(&path, &inc)?.to_string();

                    debug!(
                        "File.resolve: '{}' has included '{}'",
                        self.path.display(),
                        path.display()
                    );

                    data = data + &string;
                } else {
                    data = data + line + "\n";
                }
            }

            self.data = data;

            Ok(())
        }

        pub fn to_string(&mut self) -> String {
            self.data.clone()
        }
    }

    impl Tree {
        pub fn from_file(file: File) -> Result<Self, KickstartError> {
            Ok(Self {
                file: file,
                sections: Vec::new(),
            })
        }

        pub fn parse(mut self) -> Self {
            let mut in_section = false;

            // The command section is all data that is not in any of the other sections.
            let mut command_section = Section {
                name: "command".to_string(),
                data: String::new(),
                args: Vec::new(),
            };

            let mut section = Section {
                name: String::new(),
                data: String::new(),
                args: Vec::new(),
            };

            for line in self.file.to_string().lines() {
                if in_section {
                    if line.starts_with('%') {
                        if line == "%end" {
                            in_section = false;
                            self.sections.push(section.clone());
                            debug!("Tree.parse: end section '{}'", section.name);
                        } else {
                            // This is an error as we're encountering a new section while still being
                            // inside a section.
                            eprintln!("encountered new section while still inside section");
                        }
                    } else {
                        trace!("Tree.parse: '{}'", line);

                        section.data = section.data + line + "\n";
                    }
                } else {
                    if line.starts_with('%') {
                        if line == "%end" {
                            // This is an error as we're encountering '%end' while not being in a
                            // section
                            eprintln!("encountered %end section while not inside section");
                        } else {
                            // We're starting a new section
                            in_section = true;

                            let mut parts: Vec<String> =
                                line.split_whitespace().map(str::to_string).collect();

                            let args = parts.split_off(1);

                            section = Section {
                                name: parts[0].clone(),
                                data: String::new(),
                                args: args,
                            };

                            debug!("Tree.parse: new section '{}'", section.name);
                        }
                    } else {
                        // TODO: Skip empty lines, is this correct, empty lines might carry
                        // significance in sections but do they carry it outside of %post/%pre?
                        if line != "" {
                            command_section.data = command_section.data + line + "\n";
                        }
                    }
                }
            }

            self.sections.push(command_section);
            self.merge()
        }

        /// After parsing there can be duplicate sections, we merge these down to single sections.
        fn merge(self) -> Self {
            self
        }
    }
}

fn make_cli() -> clap::Command<'static> {
    clap::command!()
        .arg(
            clap::arg!(<src> "Kickstart input file")
                .required(true)
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            clap::arg!(<dst> "osbuild manifest output file")
                .required(true)
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            clap::arg!(-I --include "include path for kickstart files")
                .default_value(".")
                .value_hint(clap::ValueHint::DirPath),
        )
}

#[test]
fn verify_cli() {
    make_cli().debug_assert();
}

fn main() {
    stderrlog::new()
        .module(module_path!())
        .verbosity(10)
        .init()
        .unwrap();

    let matches = make_cli().get_matches();

    let src = matches.value_of("src").unwrap();
    // let dst = matches.value_of("dst").unwrap();
    let inc = matches.value_of("include").unwrap();

    // Let's verify some of these paths.
    let src_path = Path::new(src);
    // let dst_path = Path::new(dst);
    let inc_path = Path::new(inc);

    if !src_path.exists() {
        eprintln!("The path given for `src` does not exist: '{}'", src);
        exit(1);
    }

    if !src_path.is_file() {
        eprintln!("The path given for `src` is not a file: '{}'", src);
        exit(1);
    }

    if !inc_path.exists() {
        eprintln!("The path given for `include` does not exist: '{}'", inc);
        exit(1);
    }

    if !inc_path.is_dir() {
        eprintln!("The path given for `include` is not a directory: '{}'", inc);
        exit(1);
    }

    let _kickstart = kickstart::Kickstart::from_path(&src_path, &inc_path);
}
