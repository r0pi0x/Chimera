//! The object base (specs/halo_ce/objects.md, "The object base"): the
//! first 0x17C bytes of every object tag: type, flags, bounds, the model,
//! animation, collision, physics, shader and effect references, the four
//! function inputs, and the attachment, widget, function, change colour and
//! predicted resource blocks.

use std::io;

use super::function::ObjectFunction;
use super::{FunctionInput, MAX_ATTACHMENTS, MAX_CHANGE_COLORS, MAX_FUNCTIONS, ObjectType, OBJECT_BASE_SIZE};
use super::{bit_flags, object_view};
use crate::cache::{MapFile, RealRgb, TagEntry, TagId, TagRef, TagStruct, View};
use crate::scenario::coded_enum;

bit_flags! {
    /// The object flags (offset 0x002); bits 4 to 15 are zero on the disc.
    pub struct ObjectFlags(u16) {
        DOES_NOT_CAST_SHADOW = 0,
        TRANSPARENT_SELF_OCCLUSION = 1,
        /// Weapons and powerups on the disc (value 4).
        BRIGHTER_THAN_IT_SHOULD_BE = 2,
        NOT_A_PATHFINDING_OBSTACLE = 3,
    }
}

coded_enum! {
    /// A predicted resource's kind (a loading hint).
    pub enum PredictedResourceType {
        Bitmap = 0,
        /// Inferred; the disc has only bitmaps.
        Sound = 1,
    }
}

crate::tag_struct! {
    /// An attachment (element 0x48): a light, looping sound, effect,
    /// contrail or particle system on a model marker. The kind is the
    /// referenced tag's class; a null reference or another class is ignored
    /// by the game.
    pub struct Attachment: 0x48 {
        /// The attached tag.
        0x00 attached: TagRef,
        /// The model marker it sits on.
        0x10 marker_name: String,
        /// Attachment function reference: 0 none, 1 to 4 the outgoing
        /// function A to D (see `attachment_function`).
        0x30 primary_scale: i16,
        /// Same.
        0x32 secondary_scale: i16,
        /// Change colour reference: 0 none, 1 to 4 = change colour A to D.
        0x34 change_color: i16,
    }
}

crate::tag_struct! {
    /// A widget (element 0x20): a tag reference (`ant!` antenna, `mgs2`
    /// light volume, `flag`) and padding.
    pub struct Widget: 0x20 {
        0x00 widget: TagRef,
    }
}

crate::tag_struct! {
    /// One permutation of a change colour (element 0x1C).
    pub struct ChangeColorPermutation: 0x1C {
        /// Cumulative: the weights of one change colour rise to 1.0.
        0x00 weight: f32,
        0x04 color_lower_bound: RealRgb,
        0x10 color_upper_bound: RealRgb,
    }
}

crate::tag_struct! {
    /// A change colour (element 0x2C).
    pub struct ChangeColor: 0x2C {
        /// Function input index (0 none, 1 to 4 incoming A to D, 5 to 8
        /// outgoing A to D).
        0x00 darken_by: i16,
        /// Function input index.
        0x02 scale_by: i16,
        /// Bit 0 blend in HSV space, bit 1 take the long way round the hue
        /// circle.
        0x04 scale_flags: u32,
        0x08 color_lower_bound: RealRgb,
        0x14 color_upper_bound: RealRgb,
        0x20 permutations: Vec<ChangeColorPermutation>,
    }
}

impl ChangeColor {
    /// Scale flag bit 0: blend in HSV space.
    pub fn blends_in_hsv(&self) -> bool {
        self.scale_flags & 1 != 0
    }

    /// Scale flag bit 1: take the long way round the hue circle.
    pub fn long_way_round(&self) -> bool {
        self.scale_flags & 2 != 0
    }
}

crate::tag_struct! {
    /// A predicted resource (element 8): a loading hint, ignorable.
    pub struct PredictedResource: 8 {
        0x00 resource_type: PredictedResourceType,
        0x02 resource_index: i16,
        0x04 tag: TagId,
    }
}

crate::tag_struct! {
    /// The object base, every field of it (absolute offsets; `SIZE` is
    /// the layer's end). Read it from a view of the tag's main struct with
    /// `ObjectBase::read` (the tag) or `read_view`.
    pub struct ObjectBase: 0x17C {
        /// Equals the tag's class on every tag.
        0x000 object_type: ObjectType,
        0x002 flags: ObjectFlags,
        /// World units: the bounding sphere used for culling, visibility and
        /// cheap overlap tests.
        0x004 bounding_radius: f32,
        /// World units: the sphere's centre relative to the object's
        /// origin, in the object's frame.
        0x008 bounding_offset: [f32; 3],
        /// World units: the model is drawn and the object placed with this
        /// shift.
        0x014 origin_offset: [f32; 3],
        /// Multiplies the push a damage effect's acceleration gives (0 =
        /// unmoved).
        0x020 acceleration_scale: f32,
        /// Filled by the map build; bit 0: at least one change colour has a
        /// scale by function.
        0x024 runtime_flags: u32,
        /// Class `mode`.
        0x028 model: TagRef,
        /// Class `antr`.
        0x038 animation_graph: TagRef,
        /// Class `coll`.
        0x070 collision_model: TagRef,
        /// Class `phys`.
        0x080 physics: TagRef,
        /// Any shader class (`spla` on the disc): drawn over the model for
        /// shields and similar.
        0x090 modifier_shader: TagRef,
        /// Class `effe`.
        0x0A0 creation_effect: TagRef,
        /// World units: the radius used to decide whether to draw.
        0x104 render_bounding_radius: f32,
        0x108 input_a: FunctionInput,
        0x10A input_b: FunctionInput,
        0x10C input_c: FunctionInput,
        0x10E input_d: FunctionInput,
        /// Index into the HUD message text list: the prompt shown when the
        /// player can interact with the object.
        0x13C hud_text_message_index: i16,
        /// When non-zero the shader permutation the model is forced to use.
        0x13E forced_shader_permutation: i16,
        /// At most 8 are used (`MAX_ATTACHMENTS`); extra elements are cut.
        0x140 attachments: Vec<Attachment>,
        0x14C widgets: Vec<Widget>,
        /// At most 4 are used (`MAX_FUNCTIONS`); extra elements are cut.
        0x158 functions: Vec<ObjectFunction>,
        /// At most 4 are used (`MAX_CHANGE_COLORS`); extra elements are cut.
        0x164 change_colors: Vec<ChangeColor>,
        0x170 predicted_resources: Vec<PredictedResource>,
    }
}

impl ObjectBase {
    /// Read the object layer of the object tag `tag` (any class deriving
    /// from `obje`, or `plac`).
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<ObjectBase> {
        let v = object_view(map, tag, OBJECT_BASE_SIZE)?;
        Self::read_view(&v)
    }

    /// Read the layer from a view of at least 0x17C bytes at the start of
    /// the tag's main struct, cutting the blocks to the engine's limits.
    pub fn read_view(v: &View<'_>) -> io::Result<ObjectBase> {
        let mut b = <ObjectBase as TagStruct>::read(v)?;
        b.attachments.truncate(MAX_ATTACHMENTS);
        b.functions.truncate(MAX_FUNCTIONS);
        b.change_colors.truncate(MAX_CHANGE_COLORS);
        Ok(b)
    }

    /// The four incoming function sources, A to D.
    pub fn inputs(&self) -> [FunctionInput; 4] {
        [self.input_a, self.input_b, self.input_c, self.input_d]
    }
}
