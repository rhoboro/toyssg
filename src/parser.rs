use crate::constants::{TAG_BODY, TAG_DOC, TAG_ENTRY, TAG_KEY, TAG_VALUE};
use crate::models::Result;
use std::collections::HashMap;
use toypeg::{
    GrammarBuilder, MatchResult, Node, TPAny, TPChar, TPContext, TPExpr, TPMany, TPNode, TPNot,
    TPOneMany, TPOr, TPRange, TPSeq, TPTag, TPToken,
};

pub struct MarkdownParser;

impl MarkdownParser {
    pub fn parse(input: &str) -> Result<TPContext> {
        let builder = GrammarBuilder::new();
        let g = builder.clone();

        let grammar = builder
            .insert_as_node(
                "MARKDOWN",
                TPSeq::builder()
                    .seq(g.reference("FRONT_MATTER"))
                    .seq(TPNode::new(
                        TPMany::new(g.reference("BLOCK_WRAPPER")),
                        TPTag::Tag(TAG_BODY),
                    ))
                    .build(),
                TPTag::Tag(TAG_DOC),
            )
            .insert_as_node(
                "FRONT_MATTER",
                TPSeq::builder()
                    .seq(TPChar::new("---"))
                    .seq(g.reference("NEWLINE"))
                    .seq(TPMany::new(g.reference("YAML_ENTRY")))
                    .seq(TPChar::new("---"))
                    .seq(g.reference("NEWLINE"))
                    .build(),
                TPTag::Tag("#FrontMatter"),
            )
            .insert_as_node(
                "YAML_ENTRY",
                TPSeq::builder()
                    .seq(TPNode::new(g.reference("IDENTIFIER"), TPTag::Tag(TAG_KEY)))
                    .seq(TPChar::new(": "))
                    .seq(TPNode::new(g.reference("VALUE"), TPTag::Tag(TAG_VALUE)))
                    .seq(g.reference("NEWLINE"))
                    .build(),
                TPTag::Tag(TAG_ENTRY),
            )
            .insert(
                "BLOCK_WRAPPER",
                TPSeq::new(TPMany::new(g.reference("NEWLINE")), g.reference("BLOCK")),
            )
            .insert(
                "BLOCK",
                TPOr::builder()
                    .or(g.reference("HEADER"))
                    .or(g.reference("CODE_BLOCK"))
                    .or(g.reference("IMAGE"))
                    .or(g.reference("LIST"))
                    .or(g.reference("PARAGRAPH"))
                    .build()
                    .unwrap(),
            )
            .insert_as_node(
                "LIST",
                TPOneMany::new(g.reference("LIST_ITEM")),
                TPTag::Tag("#List"),
            )
            .insert_as_node(
                "LIST_ITEM",
                TPSeq::new(
                    TPChar::new("- "),
                    TPSeq::new(
                        TPNode::new(g.reference("VALUE"), TPTag::Tag("#Text")),
                        g.reference("NEWLINE"),
                    ),
                ),
                TPTag::Tag("#Item"),
            )
            .insert_as_node(
                "IMAGE",
                TPSeq::new(
                    TPChar::new("![]("),
                    TPSeq::new(
                        TPNode::new(
                            TPMany::new(TPSeq::new(
                                TPNot::new(TPChar::new(")")),
                                g.reference("ANY_CHAR"),
                            )),
                            TPTag::Tag("#Url"),
                        ),
                        TPSeq::new(TPChar::new(")"), g.reference("NEWLINE")),
                    ),
                ),
                TPTag::Tag("#Image"),
            )
            .insert_as_node(
                "HEADER",
                TPSeq::new(
                    TPNode::new(TPOneMany::new(TPChar::new("#")), TPTag::Tag("#Level")),
                    TPSeq::new(
                        TPChar::new(" "),
                        TPSeq::new(
                            TPNode::new(g.reference("VALUE"), TPTag::Tag("#Text")),
                            g.reference("NEWLINE"),
                        ),
                    ),
                ),
                TPTag::Tag("#Header"),
            )
            .insert_as_node(
                "CODE_BLOCK",
                TPSeq::new(
                    TPChar::new("```"),
                    TPSeq::new(
                        TPNode::new(g.reference("VALUE"), TPTag::Tag("#Lang")),
                        TPSeq::new(
                            g.reference("NEWLINE"),
                            TPSeq::new(
                                TPNode::new(
                                    TPMany::new(TPSeq::new(
                                        TPNot::new(TPChar::new("```")),
                                        g.reference("ANY_CHAR"),
                                    )),
                                    TPTag::Tag("#Code"),
                                ),
                                TPSeq::new(TPChar::new("```"), g.reference("NEWLINE")),
                            ),
                        ),
                    ),
                ),
                TPTag::Tag("#CodeBlock"),
            )
            .insert_as_node(
                "PARAGRAPH",
                TPOneMany::new(g.reference("PLAIN_LINE")),
                TPTag::Tag("#P"),
            )
            .insert_as_node(
                "PLAIN_LINE",
                TPSeq::new(
                    TPNot::new(
                        TPOr::builder()
                            .or(g.reference("NEWLINE"))
                            .or(TPChar::new("#"))
                            .or(TPChar::new("```"))
                            .or(TPChar::new("![]("))
                            .or(TPChar::new("- "))
                            .build()
                            .unwrap(),
                    ),
                    TPSeq::new(g.reference("VALUE"), g.reference("NEWLINE")),
                ),
                TPTag::Tag("#Line"),
            )
            .insert(
                "IDENTIFIER",
                TPOneMany::new(TPOr::new(
                    TPRange::new("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"),
                    TPRange::new("_-"),
                )),
            )
            .insert(
                "VALUE",
                TPMany::new(TPSeq::new(
                    TPNot::new(g.reference("NEWLINE")),
                    g.reference("ANY_CHAR"),
                )),
            )
            .insert("ANY_CHAR", TPAny::new())
            .insert("NEWLINE", TPOr::new(TPChar::new("\r\n"), TPChar::new("\n")))
            .build();

        let mut text = input.to_string();
        if !text.ends_with('\n') {
            text.push('\n');
        }

        let context = TPContext::new(text);

        let g_borrow = grammar.borrow();
        let start_rule = g_borrow
            .get("MARKDOWN")
            .ok_or("Root rule 'MARKDOWN' not found")?;

        match start_rule.matches(context) {
            Ok(MatchResult::Success(c)) => Ok(c),
            Ok(MatchResult::Failure(c)) => Err(format!("{:?}", c.tree).into()),
            Err(e) => {
                let err_msg = format!("Parse failed at position {}.", e);
                Err(err_msg.into())
            }
        }
    }

    pub fn extract_metadata(root: &Node) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        Self::find_metadata_entries(root, &mut metadata);
        metadata
    }

    fn find_metadata_entries(node: &Node, map: &mut HashMap<String, String>) {
        if node.tag == TPTag::Tag(TAG_ENTRY) {
            let key = Self::find_child_token_by_tag(node, TAG_KEY);
            let value = Self::find_child_token_by_tag(node, TAG_VALUE);

            if let (Some(k), Some(v)) = (key, value) {
                map.insert(k.to_string(), v.to_string().trim().to_string());
            }
        }

        for child in &node.nodes {
            Self::find_metadata_entries(&child.borrow(), map);
        }
    }

    fn find_child_token_by_tag(node: &Node, tag_str: &'static str) -> Option<TPToken> {
        node.nodes
            .iter()
            .map(|n| n.borrow())
            .find(|n| n.tag == TPTag::Tag(tag_str))
            .and_then(|n| n.token.clone())
    }
}
