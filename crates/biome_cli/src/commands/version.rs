use biome_console::fmt::Formatter;
use biome_console::{ConsoleExt, fmt, markup};
use biome_service::workspace::ServerInfo;

use crate::{CliDiagnostic, CliSession, VERSION};

/// Handle of the `version` command. Prints a more in detail version of biome.
pub(crate) fn full_version(session: CliSession) -> Result<(), CliDiagnostic> {
    session.app.console.log(markup! {
    "CLI:        "{VERSION}
    });

    if let Some(languages) = languages_of_reduced_build() {
        session.app.console.log(markup! {
        "Languages:  "{languages}
        });
    }

    match session.app.workspace.server_info() {
        None => {
            session.app.console.log(markup! {
                "Server:     "<Dim>"not connected"</Dim>
            });
        }
        Some(info) => {
            session.app.console.log(markup! {
"Server:
  Name:     "{info.name}"
  Version:  "{DisplayServerVersion(info)}
            });
        }
    };

    Ok(())
}

/// Returns the languages compiled into this binary, such as `CSS, JavaScript, JSON`, when at least
/// one optional language is left out, and `None` for a build with every language.
fn languages_of_reduced_build() -> Option<String> {
    let languages = [
        ("CSS", cfg!(feature = "lang_css")),
        ("GraphQL", cfg!(feature = "lang_graphql")),
        ("GritQL", cfg!(feature = "lang_grit")),
        ("HTML", cfg!(feature = "lang_html")),
        ("JavaScript", cfg!(feature = "lang_js")),
        ("JSON", true),
        ("Markdown", cfg!(feature = "lang_md")),
        ("YAML", cfg!(feature = "lang_yaml")),
    ];
    if languages.iter().all(|(_, compiled)| *compiled) {
        return None;
    }
    let compiled: Vec<_> = languages
        .iter()
        .filter(|(_, compiled)| *compiled)
        .map(|(name, _)| *name)
        .collect();
    Some(compiled.join(", "))
}

pub(super) struct DisplayServerVersion<'a>(pub &'a ServerInfo);

impl fmt::Display for DisplayServerVersion<'_> {
    fn fmt(&self, fmt: &mut Formatter) -> std::io::Result<()> {
        match &self.0.version {
            None => markup!(<Dim>"-"</Dim>).fmt(fmt),
            Some(version) => {
                write!(fmt, "{version}")
            }
        }
    }
}
