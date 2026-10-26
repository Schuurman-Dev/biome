#![cfg_attr(
    not(any(feature = "lang_css", feature = "lang_graphql")),
    expect(
        dead_code,
        reason = "Without CSS or GraphQL, JavaScript has no language to embed."
    )
)]

use super::EmbedContent;
#[cfg(feature = "lang_css")]
use biome_languages::CssFileSource;
use biome_languages::DocumentFileSource;
#[cfg(feature = "lang_graphql")]
use biome_languages::GraphqlFileSource;
use biome_rowan::{TextRange, TokenText};

/// Language embedded in JavaScript source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GuestLanguage {
    #[cfg(feature = "lang_css")]
    Css,
    #[cfg(feature = "lang_graphql")]
    GraphQL,
}

impl From<GuestLanguage> for DocumentFileSource {
    fn from(value: GuestLanguage) -> Self {
        match value {
            #[cfg(feature = "lang_css")]
            GuestLanguage::Css => CssFileSource::css().into(),
            #[cfg(feature = "lang_graphql")]
            GuestLanguage::GraphQL => GraphqlFileSource::graphql().into(),
        }
    }
}

/// Describes where a potential embedded language can be found in JavaScript.
pub(crate) enum EmbedCandidate {
    TaggedTemplate {
        tag: TemplateTagKind,
        content: EmbedContent,
        /// Ranges of the `${...}` interpolations in the template.
        interpolations: Vec<TextRange>,
    },
    JsxStyleAttribute {
        content: EmbedContent,
    },
}

impl EmbedCandidate {
    pub fn content(&self) -> EmbedContent {
        match self {
            Self::TaggedTemplate { content, .. } | Self::JsxStyleAttribute { content } => {
                content.clone()
            }
        }
    }
}

/// Describes how a JavaScript template tag was classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TemplateTagKind {
    Identifier(TokenText),
    MemberExpression {
        object: TokenText,
        property: TokenText,
    },
    CallExpression {
        callee: TokenText,
    },
}

#[derive(Debug)]
pub(crate) struct EmbedMatch {
    pub guest: GuestLanguage,
}

pub(crate) struct EmbedDetectorsRegistry;

impl EmbedDetectorsRegistry {
    pub fn detect_match(
        candidate: &EmbedCandidate,
        file_source: &DocumentFileSource,
    ) -> Option<EmbedMatch> {
        if matches!(candidate, EmbedCandidate::JsxStyleAttribute { .. }) {
            return cfg_select! {
                feature = "lang_css" => Some(EmbedMatch {
                    guest: GuestLanguage::Css,
                }),
                _ => None,
            };
        }
        for detector in JS_DETECTORS.iter() {
            if let Some(guest) = detector.try_match(candidate, file_source) {
                return Some(EmbedMatch { guest });
            }
        }
        None
    }
}

enum EmbedDetector {
    TemplateTag {
        tag: &'static str,
        target: EmbedTarget,
    },
    TemplateExpression {
        object: &'static str,
        target: EmbedTarget,
    },
}

impl EmbedDetector {
    fn try_match(
        &self,
        candidate: &EmbedCandidate,
        file_source: &DocumentFileSource,
    ) -> Option<GuestLanguage> {
        match (self, candidate) {
            (
                Self::TemplateTag { tag, target },
                EmbedCandidate::TaggedTemplate {
                    tag: TemplateTagKind::Identifier(name),
                    ..
                },
            ) => {
                if name.text() == *tag {
                    target.resolve(candidate, file_source)
                } else {
                    None
                }
            }
            (
                Self::TemplateExpression { object, target },
                EmbedCandidate::TaggedTemplate { tag, .. },
            ) => match tag {
                TemplateTagKind::MemberExpression { object: obj, .. } => {
                    if obj.text() == *object {
                        target.resolve(candidate, file_source)
                    } else {
                        None
                    }
                }
                TemplateTagKind::CallExpression { callee } => {
                    if callee.text() == *object {
                        target.resolve(candidate, file_source)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

enum EmbedTarget {
    Static(GuestLanguage),
}

impl EmbedTarget {
    fn resolve(
        &self,
        _candidate: &EmbedCandidate,
        _file_source: &DocumentFileSource,
    ) -> Option<GuestLanguage> {
        match self {
            Self::Static(guest) => Some(*guest),
        }
    }
}

static JS_DETECTORS: &[EmbedDetector] = &[
    #[cfg(feature = "lang_css")]
    EmbedDetector::TemplateTag {
        tag: "css",
        target: EmbedTarget::Static(GuestLanguage::Css),
    },
    #[cfg(feature = "lang_css")]
    EmbedDetector::TemplateExpression {
        object: "styled",
        target: EmbedTarget::Static(GuestLanguage::Css),
    },
    #[cfg(feature = "lang_graphql")]
    EmbedDetector::TemplateTag {
        tag: "gql",
        target: EmbedTarget::Static(GuestLanguage::GraphQL),
    },
    #[cfg(feature = "lang_graphql")]
    EmbedDetector::TemplateTag {
        tag: "graphql",
        target: EmbedTarget::Static(GuestLanguage::GraphQL),
    },
    #[cfg(feature = "lang_graphql")]
    EmbedDetector::TemplateExpression {
        object: "graphql",
        target: EmbedTarget::Static(GuestLanguage::GraphQL),
    },
];
