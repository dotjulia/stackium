pub fn tag_to_string(tag: gimli::DwTag) -> String {
    match tag {
        gimli::DW_TAG_array_type => "DW_TAG_array_type",
        gimli::DW_TAG_class_type => "DW_TAG_class_type",
        gimli::DW_TAG_entry_point => "DW_TAG_entry_point",
        gimli::DW_TAG_enumeration_type => "DW_TAG_enumeration_type",
        gimli::DW_TAG_formal_parameter => "DW_TAG_formal_parameter",
        gimli::DW_TAG_imported_declaration => "DW_TAG_imported_declaration",
        gimli::DW_TAG_label => "DW_TAG_label",
        gimli::DW_TAG_lexical_block => "DW_TAG_lexical_block",
        gimli::DW_TAG_member => "DW_TAG_member",
        gimli::DW_TAG_pointer_type => "DW_TAG_pointer_type",
        gimli::DW_TAG_reference_type => "DW_TAG_reference_type",
        gimli::DW_TAG_compile_unit => "DW_TAG_compile_unit",
        gimli::DW_TAG_string_type => "DW_TAG_string_type",
        gimli::DW_TAG_structure_type => "DW_TAG_structure_type",
        gimli::DW_TAG_subroutine_type => "DW_TAG_subroutine_type",
        gimli::DW_TAG_typedef => "DW_TAG_typedef",
        gimli::DW_TAG_union_type => "DW_TAG_union_type",
        gimli::DW_TAG_unspecified_parameters => "DW_TAG_unspecified_parameters",
        gimli::DW_TAG_variant => "DW_TAG_variant",
        gimli::DW_TAG_common_block => "DW_TAG_common_block",
        gimli::DW_TAG_common_inclusion => "DW_TAG_common_inclusion",
        gimli::DW_TAG_inheritance => "DW_TAG_inheritance",
        gimli::DW_TAG_inlined_subroutine => "DW_TAG_inlined_subroutine",
        gimli::DW_TAG_module => "DW_TAG_module",
        gimli::DW_TAG_ptr_to_member_type => "DW_TAG_ptr_to_member_type",
        gimli::DW_TAG_set_type => "DW_TAG_set_type",
        gimli::DW_TAG_subrange_type => "DW_TAG_subrange_type",
        gimli::DW_TAG_with_stmt => "DW_TAG_with_stmt",
        gimli::DW_TAG_access_declaration => "DW_TAG_access_declaration",
        gimli::DW_TAG_base_type => "DW_TAG_base_type",
        gimli::DW_TAG_catch_block => "DW_TAG_catch_block",
        gimli::DW_TAG_const_type => "DW_TAG_const_type",
        gimli::DW_TAG_constant => "DW_TAG_constant",
        gimli::DW_TAG_enumerator => "DW_TAG_enumerator",
        gimli::DW_TAG_file_type => "DW_TAG_file_type",
        gimli::DW_TAG_friend => "DW_TAG_friend",
        gimli::DW_TAG_namelist => "DW_TAG_namelist",
        gimli::DW_TAG_namelist_item => "DW_TAG_namelist_item",
        gimli::DW_TAG_packed_type => "DW_TAG_packed_type",
        gimli::DW_TAG_subprogram => "DW_TAG_subprogram",
        gimli::DW_TAG_template_type_parameter => "DW_TAG_template_type_parameter",
        gimli::DW_TAG_template_value_parameter => "DW_TAG_template_value_parameter",
        gimli::DW_TAG_thrown_type => "DW_TAG_thrown_type",
        gimli::DW_TAG_try_block => "DW_TAG_try_block",
        gimli::DW_TAG_variant_part => "DW_TAG_variant_part",
        gimli::DW_TAG_variable => "DW_TAG_variable",
        gimli::DW_TAG_volatile_type => "DW_TAG_volatile_type",
        gimli::DW_TAG_dwarf_procedure => "DW_TAG_dwarf_procedure",
        gimli::DW_TAG_restrict_type => "DW_TAG_restrict_type",
        gimli::DW_TAG_interface_type => "DW_TAG_interface_type",
        gimli::DW_TAG_namespace => "DW_TAG_namespace",
        gimli::DW_TAG_imported_module => "DW_TAG_imported_module",
        gimli::DW_TAG_unspecified_type => "DW_TAG_unspecified_type",
        gimli::DW_TAG_partial_unit => "DW_TAG_partial_unit",
        gimli::DW_TAG_imported_unit => "DW_TAG_imported_unit",
        gimli::DW_TAG_condition => "DW_TAG_condition",
        gimli::DW_TAG_shared_type => "DW_TAG_shared_type",
        gimli::DW_TAG_type_unit => "DW_TAG_type_unit",
        gimli::DW_TAG_rvalue_reference_type => "DW_TAG_rvalue_reference_type",
        gimli::DW_TAG_template_alias => "DW_TAG_template_alias",
        gimli::DW_TAG_lo_user => "DW_TAG_lo_user",
        gimli::DW_TAG_hi_user => "DW_TAG_hi_user",
        _ => "Unknown tag",
    }
    .to_owned()
}

pub fn dw_at_to_string(attr: gimli::DwAt) -> String {
    match attr {
        gimli::DW_AT_sibling => "DW_AT_sibling",
        gimli::DW_AT_location => "DW_AT_location",
        gimli::DW_AT_name => "DW_AT_name",
        gimli::DW_AT_ordering => "DW_AT_ordering",
        gimli::DW_AT_byte_size => "DW_AT_byte_size",
        gimli::DW_AT_bit_offset => "DW_AT_bit_offset",
        gimli::DW_AT_bit_size => "DW_AT_bit_size",
        gimli::DW_AT_stmt_list => "DW_AT_stmt_list",
        gimli::DW_AT_low_pc => "DW_AT_low_pc",
        gimli::DW_AT_high_pc => "DW_AT_high_pc",
        gimli::DW_AT_language => "DW_AT_language",
        gimli::DW_AT_discr => "DW_AT_discr",
        gimli::DW_AT_discr_value => "DW_AT_discr_value",
        gimli::DW_AT_visibility => "DW_AT_visibility",
        gimli::DW_AT_import => "DW_AT_import",
        gimli::DW_AT_string_length => "DW_AT_string_length",
        gimli::DW_AT_common_reference => "DW_AT_common_reference",
        gimli::DW_AT_comp_dir => "DW_AT_comp_dir",
        gimli::DW_AT_const_value => "DW_AT_const_value",
        gimli::DW_AT_containing_type => "DW_AT_containing_type",
        gimli::DW_AT_default_value => "DW_AT_default_value",
        gimli::DW_AT_inline => "DW_AT_inline",
        gimli::DW_AT_is_optional => "DW_AT_is_optional",
        gimli::DW_AT_lower_bound => "DW_AT_lower_bound",
        gimli::DW_AT_producer => "DW_AT_producer",
        gimli::DW_AT_prototyped => "DW_AT_prototyped",
        gimli::DW_AT_return_addr => "DW_AT_return_addr",
        gimli::DW_AT_start_scope => "DW_AT_start_scope",
        gimli::DW_AT_bit_stride => "DW_AT_bit_stride",
        gimli::DW_AT_upper_bound => "DW_AT_upper_bound",
        gimli::DW_AT_abstract_origin => "DW_AT_abstract_origin",
        gimli::DW_AT_accessibility => "DW_AT_accessibility",
        gimli::DW_AT_address_class => "DW_AT_address_class",
        gimli::DW_AT_artificial => "DW_AT_artificial",
        gimli::DW_AT_base_types => "DW_AT_base_types",
        gimli::DW_AT_calling_convention => "DW_AT_calling_convention",
        gimli::DW_AT_count => "DW_AT_count",
        gimli::DW_AT_data_member_location => "DW_AT_data_member_location",
        gimli::DW_AT_decl_column => "DW_AT_decl_column",
        gimli::DW_AT_decl_file => "DW_AT_decl_file",
        gimli::DW_AT_decl_line => "DW_AT_decl_line",
        gimli::DW_AT_declaration => "DW_AT_declaration",
        gimli::DW_AT_discr_list => "DW_AT_discr_list",
        gimli::DW_AT_encoding => "DW_AT_encoding",
        gimli::DW_AT_external => "DW_AT_external",
        gimli::DW_AT_frame_base => "DW_AT_frame_base",
        gimli::DW_AT_friend => "DW_AT_friend",
        gimli::DW_AT_identifier_case => "DW_AT_identifier_case",
        gimli::DW_AT_macro_info => "DW_AT_macro_info",
        gimli::DW_AT_namelist_item => "DW_AT_namelist_item",
        gimli::DW_AT_priority => "DW_AT_priority",
        gimli::DW_AT_segment => "DW_AT_segment",
        gimli::DW_AT_specification => "DW_AT_specification",
        gimli::DW_AT_static_link => "DW_AT_static_link",
        gimli::DW_AT_type => "DW_AT_type",
        gimli::DW_AT_use_location => "DW_AT_use_location",
        gimli::DW_AT_variable_parameter => "DW_AT_variable_parameter",
        gimli::DW_AT_virtuality => "DW_AT_virtuality",
        gimli::DW_AT_vtable_elem_location => "DW_AT_vtable_elem_location",
        gimli::DW_AT_allocated => "DW_AT_allocated",
        gimli::DW_AT_associated => "DW_AT_associated",
        gimli::DW_AT_data_location => "DW_AT_data_location",
        gimli::DW_AT_byte_stride => "DW_AT_byte_stride",
        gimli::DW_AT_entry_pc => "DW_AT_entry_pc",
        gimli::DW_AT_use_UTF8 => "DW_AT_use_UTF8",
        gimli::DW_AT_extension => "DW_AT_extension",
        gimli::DW_AT_ranges => "DW_AT_ranges",
        gimli::DW_AT_trampoline => "DW_AT_trampoline",
        gimli::DW_AT_call_column => "DW_AT_call_column",
        gimli::DW_AT_call_file => "DW_AT_call_file",
        gimli::DW_AT_call_line => "DW_AT_call_line",
        gimli::DW_AT_description => "DW_AT_description",
        gimli::DW_AT_binary_scale => "DW_AT_binary_scale",
        gimli::DW_AT_decimal_scale => "DW_AT_decimal_scale",
        gimli::DW_AT_small => "DW_AT_small",
        gimli::DW_AT_decimal_sign => "DW_AT_decimal_sign",
        gimli::DW_AT_digit_count => "DW_AT_digit_count",
        gimli::DW_AT_picture_string => "DW_AT_picture_string",
        gimli::DW_AT_mutable => "DW_AT_mutable",
        gimli::DW_AT_threads_scaled => "DW_AT_threads_scaled",
        gimli::DW_AT_explicit => "DW_AT_explicit",
        gimli::DW_AT_object_pointer => "DW_AT_object_pointer",
        gimli::DW_AT_endianity => "DW_AT_endianity",
        gimli::DW_AT_elemental => "DW_AT_elemental",
        gimli::DW_AT_pure => "DW_AT_pure",
        gimli::DW_AT_recursive => "DW_AT_recursive",
        gimli::DW_AT_signature => "DW_AT_signature",
        gimli::DW_AT_main_subprogram => "DW_AT_main_subprogram",
        gimli::DW_AT_data_bit_offset => "DW_AT_data_bit_offset",
        gimli::DW_AT_const_expr => "DW_AT_const_expr",
        gimli::DW_AT_enum_class => "DW_AT_enum_class",
        gimli::DW_AT_linkage_name => "DW_AT_linkage_name",
        gimli::DW_AT_lo_user => "DW_AT_lo_user",
        gimli::DW_AT_hi_user => "DW_AT_hi_user",
        _ => "Unknown",
    }
    .to_owned()
}

use nix::libc::user_regs_struct;
use stackium_shared::Registers;

trait FromUserRegsStruct {
    fn from(value: user_regs_struct) -> Registers;
}

impl FromUserRegsStruct for Registers {
    fn from(value: user_regs_struct) -> Self {
        Registers {
            r15: value.r15,
            r14: value.r14,
            r13: value.r13,
            r12: value.r12,
            rbp: value.rbp,
            rbx: value.rbx,
            r11: value.r11,
            r10: value.r10,
            r9: value.r9,
            r8: value.r8,
            rax: value.rax,
            rcx: value.rcx,
            rdx: value.rdx,
            rsi: value.rsi,
            rdi: value.rdi,
            orig_rax: value.orig_rax,
            rip: value.rip,
            cs: value.cs,
            eflags: value.eflags,
            rsp: value.rsp,
            ss: value.ss,
            fs_base: value.fs_base,
            gs_base: value.gs_base,
            ds: value.ds,
            es: value.es,
            fs: value.fs,
            gs: value.gs,
        }
    }
}
