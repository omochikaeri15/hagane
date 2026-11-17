use std::io::Cursor;

use resand::res_value::{ResValue, ResValueType};
use resand::xmltree::{XMLTree, XMLTreeNode};

const ROOT: &str = "manifest";
const APPLICATION: &str = "application";
const META: &str = "meta-data";
const PACKAGE: &str = "package";
const SPLIT: &str = "split";
const FEATURE: &str = "isFeatureSplit";
const NAME: &str = "name";
const VALUE: &str = "value";
const RESOURCE: &str = "resource";
const NAME_ID: u32 = 0x0101_0003;
const VALUE_ID: u32 = 0x0101_0024;
const ROOT_MARKS: [&str; 3] = ["requiredSplitTypes", "splitTypes", "isSplitRequired"];
const APPLICATION_MARK: &str = "isSplitRequired";
const LISTING: &str = "com.android.vending.splits";
const MARKS: [&str; 5] = [
    "com.android.vending.splits.required",
    "com.android.stamp.source",
    "com.android.stamp.type",
    LISTING,
    "com.android.vending.derived.apk.id",
];
const FUSED: &str = "com.android.dynamic.apk.fused.modules";
const SEPARATOR: &str = ",";

pub(super) struct Identity {
    pub(super) package: String,
    pub(super) split: Option<String>,
    pub(super) feature: bool,
}

pub(super) struct Cleaned {
    pub(super) bytes: Vec<u8>,
    pub(super) listing: Option<u32>,
}

fn read(bytes: &[u8]) -> Result<XMLTree, String> {
    XMLTree::read(&mut Cursor::new(bytes)).map_err(|error| format!("AndroidManifest.xml could not be read: {error}"))
}

fn text(node: &XMLTreeNode, name: &str, tree: &XMLTree) -> Option<String> {
    match &node.get_attribute(name, &tree.string_pool)?.typed_value.data {
        ResValueType::String(held) => held.resolve(&tree.string_pool).map(str::to_string),
        _ => None,
    }
}

pub(super) fn identity(bytes: &[u8]) -> Result<Identity, String> {
    let tree = read(bytes)?;
    let root = tree.root.get_element(&[ROOT], &tree.string_pool).ok_or("AndroidManifest.xml has no manifest element")?;
    let feature = root.get_attribute(FEATURE, &tree.string_pool).is_some_and(|held| match held.typed_value.data {
        ResValueType::IntBoolean(flag) => bool::from(flag),
        _ => false,
    });

    Ok(Identity {
        package: text(root, PACKAGE, &tree).ok_or("AndroidManifest.xml names no package")?,
        split: text(root, SPLIT, &tree).filter(|held| !held.is_empty()),
        feature,
    })
}

pub(super) fn clean(bytes: &[u8], fused: &[String]) -> Result<Cleaned, String> {
    let mut tree = read(bytes)?;
    let pool = &mut tree.string_pool;
    let root = tree.root.get_element_mut(&[ROOT], pool).ok_or("AndroidManifest.xml has no manifest element")?;

    root.element.attributes.retain(|held| held.name.resolve(pool).is_none_or(|name| !ROOT_MARKS.contains(&name)));

    let Some(application) = tree.root.get_element_mut(&[ROOT, APPLICATION], pool) else {
        return write(tree, None);
    };

    application.element.attributes.retain(|held| held.name.resolve(pool).is_none_or(|name| name != APPLICATION_MARK));

    let named = |node: &XMLTreeNode, pool: &_| {
        (node.element.name.resolve(pool) == Some(META))
            .then(|| node.get_attribute(NAME, pool))
            .flatten()
            .and_then(|held| match &held.typed_value.data {
                ResValueType::String(held) => held.resolve(pool).map(str::to_string),
                _ => None,
            })
    };
    let listing = application
        .children
        .iter()
        .find(|node| named(node, pool).as_deref() == Some(LISTING))
        .and_then(|node| node.get_attribute(RESOURCE, pool))
        .and_then(|held| match held.typed_value.data {
            ResValueType::Reference(target) => Some(u32::from(target)),
            _ => None,
        });

    application.children.retain(|node| named(node, pool).is_none_or(|name| !MARKS.contains(&name.as_str())));

    if fused.is_empty() {
        return write(tree, listing);
    }

    let held = application.children.iter_mut().find(|node| named(node, pool).as_deref() == Some(FUSED));

    if let Some(node) = held {
        let before = node.get_attribute(VALUE, pool).and_then(|held| match &held.typed_value.data {
            ResValueType::String(held) => held.resolve(pool).map(str::to_string),
            _ => None,
        });
        let mut modules: Vec<&str> = before.as_deref().map_or_else(Vec::new, |held| held.split(SEPARATOR).filter(|name| !name.is_empty()).collect());

        modules.extend(fused.iter().map(String::as_str).filter(|name| !before.as_deref().is_some_and(|held| held.split(SEPARATOR).any(|known| known == *name))));

        if let Some(value) = node.get_attribute_mut(VALUE, pool) {
            value.write_string(modules.join(SEPARATOR).into(), pool);
        }

        return write(tree, listing);
    }

    let space = application.element.attributes.first().map(|held| held.ns);
    let mut node = XMLTreeNode::new().with_name_str(META.into(), pool).with_line_number(0);

    for (name, id, text) in [(NAME, NAME_ID, FUSED.to_string()), (VALUE, VALUE_ID, fused.join(SEPARATOR))] {
        let placed = node.insert_attribute(name.into(), ResValue::new_str(text.into(), pool), pool, tree.resource_map.as_mut(), Some(id.into()));

        if let Some(space) = space {
            placed.ns = space;
        }
    }

    application.add_child(node);

    write(tree, listing)
}

fn write(tree: XMLTree, listing: Option<u32>) -> Result<Cleaned, String> {
    let mut bytes = Cursor::new(Vec::new());

    tree.write(&mut bytes).map_err(|error| format!("AndroidManifest.xml could not be written: {error}"))?;

    Ok(Cleaned { bytes: bytes.into_inner(), listing })
}
