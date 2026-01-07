use std::cell::{Ref, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use anyhow::{bail, Result};
use melior::dialect::func::{self};
use melior::dialect::llvm::attributes::Linkage;
use melior::dialect::llvm::{self};
use melior::dialect::{arith, scf, DialectRegistry};
use melior::ir::attribute::{
    DenseI64ArrayAttribute,
    FlatSymbolRefAttribute,
    IntegerAttribute,
    StringAttribute,
    TypeAttribute,
};
use melior::ir::operation::{OperationBuilder, OperationLike};
use melior::ir::r#type::{FunctionType, IntegerType, MemRefType};
use melior::ir::{
    Attribute,
    Block,
    BlockLike,
    BlockRef,
    Identifier,
    Location,
    Module,
    Operation,
    OperationRef,
    Region,
    RegionLike,
    Type,
    Value,
};
use melior::pass::{self};
use melior::utility::{register_all_dialects, register_all_llvm_translations};
use melior::{Context, ExecutionEngine};
use tracing::debug;

use crate::analysis::type_evaluation::IrProgramTypes;
use crate::ast::identifiers::FunctionDeclarationID;
use crate::ast::nodes;
use crate::ast::nodes::FunctionKeyword;
use crate::backend::mlir::intrinsics::{
    generate_intrinsic_call,
    generate_resultless_intrinsic_call,
};
use crate::control_flow_graph::ControlFlowGraph;
use crate::ir::{self, AnnotatedAssignment, BlockId, FunctionId, Instruction, IrProgram, Ssaid};

pub struct MlirGenerationConfig {
    pub program: IrProgram,
    pub verify_mlir: bool,
    pub program_types: BTreeMap<FunctionDeclarationID, IrProgramTypes>,
}

struct ArithOperationVaribles {
    left_hand_side: Ssaid,
    right_hand_side: Ssaid,
    reciever: Ssaid,
}

// TODO: Figure out how we can share the module generation code without dropping references.

pub fn generate_mlir(config: MlirGenerationConfig) -> Result<ExecutionEngine> {
    let context = prepare_mlir_context();
    let mut module = Module::new(melior::ir::Location::unknown(&context));
    let mut code_gen = Box::new(CodeGen::new(
        &context,
        &module,
        config.program,
        config.program_types,
    ));

    code_gen.gen_code()?;

    run_mlir_passes(&context, &mut module);

    if config.verify_mlir {
        assert!(module.as_operation().verify());
    }

    let engine = ExecutionEngine::new(&module, 0, &[], true);

    Ok(engine)
}

fn prepare_mlir_context() -> Context {
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);

    let context = Context::new();
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    register_all_llvm_translations(&context);

    context.attach_diagnostic_handler(|diagnostic| {
        eprintln!("{}", diagnostic);
        true
    });

    context
}

fn run_mlir_passes(context: &Context, module: &mut Module) {
    let pass_manager = pass::PassManager::new(context);
    pass_manager.add_pass(pass::conversion::create_func_to_llvm());

    pass_manager.add_pass(pass::conversion::create_arith_to_llvm());
    pass_manager.add_pass(pass::conversion::create_index_to_llvm());
    pass_manager.add_pass(pass::conversion::create_scf_to_control_flow());
    pass_manager.add_pass(pass::conversion::create_control_flow_to_llvm());
    pass_manager.add_pass(pass::conversion::create_finalize_mem_ref_to_llvm());
    pass_manager.add_pass(pass::conversion::create_index_to_llvm());
    pass_manager.add_pass(pass::conversion::create_reconcile_unrealized_casts());

    debug!("MLIR output: {}", module.as_operation());
    pass_manager.run(module).unwrap();
}

use crate::types::{self, SignedIntegerType, UnsignedIntegerType};

fn get_variable_mlir_type<'c, 'a>(
    context: &'c Context,
    types: &IrProgramTypes,
    ssa_id: &Ssaid,
) -> melior::ir::Type<'a>
where
    'c: 'a,
{
    if types.is_projection(ssa_id) {
        llvm::r#type::pointer(context, 0)
    } else {
        as_mlir_type(types.lookup_variable_type(*ssa_id).unwrap(), context, types)
    }
}

pub const PLATFORM_BIT_WIDTH: usize = std::mem::size_of::<usize>() * 8; // TODO: Check if there is a builtin way to get the bit width.

pub fn as_mlir_type<'c, 'a>(
    fusion_type: types::Type,
    context: &'c Context,
    types: &IrProgramTypes,
) -> melior::ir::Type<'a>
where
    'c: 'a,
{
    match fusion_type {
        types::Type::Pointer => llvm::r#type::pointer(context, 0),
        types::Type::String => llvm::r#type::pointer(context, 0),
        types::Type::UnsignedInteger(UnsignedIntegerType(types::IntegerBitWidth::Bit8)) => {
            IntegerType::new(context, 8).into()
        }
        types::Type::UnsignedInteger(UnsignedIntegerType(types::IntegerBitWidth::Bit16)) => {
            IntegerType::new(context, 16).into()
        }
        types::Type::UnsignedInteger(UnsignedIntegerType(types::IntegerBitWidth::Bit32)) => {
            IntegerType::new(context, 32).into()
        }
        types::Type::UnsignedInteger(UnsignedIntegerType(types::IntegerBitWidth::Bit64)) => {
            IntegerType::new(context, 64).into()
        }
        types::Type::UnsignedInteger(UnsignedIntegerType(types::IntegerBitWidth::PlatformSize)) => {
            IntegerType::new(context, PLATFORM_BIT_WIDTH as u32).into()
        }
        types::Type::Integer(SignedIntegerType(types::IntegerBitWidth::Bit8)) => {
            IntegerType::new(context, 8).into()
        }
        types::Type::Integer(SignedIntegerType(types::IntegerBitWidth::Bit16)) => {
            IntegerType::new(context, 16).into()
        }
        types::Type::Integer(SignedIntegerType(types::IntegerBitWidth::Bit32)) => {
            IntegerType::new(context, 32).into()
        }
        types::Type::Integer(SignedIntegerType(types::IntegerBitWidth::Bit64)) => {
            IntegerType::new(context, 64).into()
        }
        types::Type::Integer(SignedIntegerType(types::IntegerBitWidth::PlatformSize)) => {
            IntegerType::new(context, PLATFORM_BIT_WIDTH as u32).into()
        }
        types::Type::Boolean => IntegerType::new(context, 1).into(),
        types::Type::Unit => llvm::r#type::void(context),
        types::Type::Array(array_type_id) => {
            let array_type = types.array_types.get(array_type_id).unwrap();
            let fusion_element_type = types.comp_time_types.get(&array_type.element_type).unwrap();
            let element_type = as_mlir_type(*fusion_element_type, context, types);

            llvm::r#type::array(element_type, array_type.length as u32)
        }
        types::Type::Struct(struct_type_id) => {
            let struct_type = types.struct_types.get(struct_type_id).unwrap();
            let field_types: Vec<melior::ir::Type> = struct_type
                .field_types
                .iter()
                .map(|field_type_id| {
                    let field_fusion_type = *types.comp_time_types.get(field_type_id).unwrap();
                    as_mlir_type(field_fusion_type, context, types)
                })
                .collect();

            llvm::r#type::r#struct(context, &field_types, true)
        }
        _ => todo!("unimplemented type to mlir type {:?}", fusion_type),
    }
}

pub fn as_memref_type<'c, 'a>(
    fusion_type: types::Type,
    context: &'c Context,
    types: &IrProgramTypes,
) -> MemRefType<'a>
where
    'c: 'a,
{
    match fusion_type {
        types::Type::Array(array_type_id) => {
            let array_type = types.array_types.get(array_type_id).unwrap();
            let fusion_element_type = types.comp_time_types.get(&array_type.element_type).unwrap();
            let element_type = as_mlir_type(*fusion_element_type, context, types);

            MemRefType::new(element_type, &[array_type.length as i64], None, None)
        }
        _ => todo!("unimplemented type to memref type {:?}", fusion_type),
    }
}

// TODO: move this into a struct with context

#[derive(Copy, Clone)]
pub struct MlirBlockId(pub usize);

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    module: &'ctx Module<'ctx>,
    annon_string_counter: RefCell<usize>,
    program: IrProgram,
    program_types: BTreeMap<FunctionDeclarationID, IrProgramTypes>,
    current_fn_decl_id: FunctionDeclarationID,
}

impl<'ctx> CodeGen<'ctx> {
    fn new(
        context: &'ctx Context,
        module: &'ctx Module<'ctx>,
        ir_program: IrProgram,
        ir_types: BTreeMap<FunctionDeclarationID, IrProgramTypes>,
    ) -> Self {
        Self {
            context,
            module,
            annon_string_counter: 0.into(),
            current_fn_decl_id: ir_program.entry_function_id,
            program: ir_program,
            program_types: ir_types,
        }
    }

    fn gen_code(&mut self) -> Result<()> {
        debug!("generating code");

        for function_decl_id in self.program.external_function_declaraitons.iter() {
            let decl = self.declare_function(function_decl_id)?;
            self.module.body().append_operation(decl);
        }

        for (function_decl_id, cfg) in self.program.control_flow_graphs.iter() {
            self.current_fn_decl_id = *function_decl_id;
            let decl = self.gen_function(function_decl_id, cfg, &self.program.blocks)?;
            self.module.body().append_operation(decl);
        }

        Ok(())
    }

    fn declare_function(&self, function_decl_id: &FunctionDeclarationID) -> Result<Operation<'_>> {
        let function_declaration = self
            .program
            .node_db
            .function_declarations
            .get(function_decl_id)
            .unwrap();

        let argument_types = function_declaration
            .argument_types()
            .map(|r#type| {
                as_mlir_type(
                    self.program_types[&self.program.entry_function_id]
                        .lookup_type_name_type(r#type)
                        .unwrap(),
                    self.context,
                    &self.program_types[&self.program.entry_function_id],
                )
            }) // TODO: need to actually types available to function declarations.
            .collect::<Vec<Type<'ctx>>>();
        let function_region = Region::new();
        let location = melior::ir::Location::unknown(self.context);

        if function_declaration.is_external() {
            let return_type = as_mlir_type(
                self.program_types[&self.program.entry_function_id]
                    .lookup_type_name_type(&function_declaration.get_return_type())
                    .unwrap(),
                self.context,
                &self.program_types[&self.program.entry_function_id],
            );

            Ok(llvm::func(
                self.context,
                StringAttribute::new(self.context, &function_declaration.identifier.0),
                TypeAttribute::new(llvm::r#type::function(
                    return_type,
                    argument_types.as_slice(),
                    false,
                )),
                function_region,
                &[
                    (
                        Identifier::new(self.context, "sym_visibility"),
                        StringAttribute::new(self.context, "private").into(),
                    ),
                    (
                        Identifier::new(self.context, "llvm.emit_c_interface"),
                        Attribute::unit(self.context),
                    ),
                ],
                location,
            ))
        } else {
            bail!("Function declaration is not external")
        }
    }

    pub fn append_operation<'c, 'a>(
        &self,
        block_id: usize,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        operation: Operation<'c>,
    ) -> OperationRef<'c, 'a> {
        let block = block_references[&block_id];

        block.append_operation(operation)
    }

    fn gen_load_from_stack_operation<'c, 'a>(
        &self,
        current_block_id: usize,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        ptr: Value<'c, '_>,
        mlir_type: Type<'c>,
        location: Location<'c>,
    ) -> OperationRef<'c, 'a>
    where
        'ctx: 'c,
    {
        let load_operation = llvm::load(self.context, ptr, mlir_type, location, Default::default());

        self.append_operation(current_block_id, block_references, load_operation)
    }

    fn gen_allocate_on_stack_operation<'c, 'a>(
        &self,
        current_block_id: usize,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        mlir_type: Type<'c>,
        location: Location<'c>,
    ) -> OperationRef<'c, 'a>
    where
        'ctx: 'c,
    {
        // The type of the pointer returned from allocation. The recommendation seems to be to
        // stick to untyped pointers in LLVM.
        let ptr_type = melior::dialect::llvm::r#type::pointer(self.context, 0);

        // The number of items to be allocated. This function assumes allocating
        // a single item of type `mlir_type`.
        let constant_one: Value = self
            .append_operation(
                current_block_id,
                block_references,
                melior::dialect::arith::constant(
                    self.context,
                    IntegerAttribute::new(IntegerType::new(self.context, 64).into(), 1_i64).into(),
                    Location::unknown(self.context),
                ),
            )
            .result(0)
            .unwrap()
            .into();

        // The type of allocated item is set using `AllocaOptions`. Note this is not the type
        // of the result. Rather this informs the compiler of the required size of the allocated
        // memory on the stack.
        let allocation_options =
            llvm::AllocaOptions::new().elem_type(Some(TypeAttribute::new(mlir_type)));

        let alloca_operation = llvm::alloca(
            self.context,
            constant_one,
            ptr_type,
            location,
            allocation_options,
        );

        self.append_operation(current_block_id, block_references, alloca_operation)
    }

    pub fn save_value_to_variable<'c, 'a>(
        &self,
        current_block_id: usize,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        value: Value<'c, 'a>,
        variable_id: &Ssaid,
        variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
        location: Location<'c>,
    ) {
        let variable_pointer = variable_store[variable_id];
        self.gen_store_operation(
            current_block_id,
            block_references,
            value,
            variable_pointer,
            location,
        );
    }

    pub fn gen_store_operation<'c, 'a>(
        &self,
        current_block_id: usize,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        value: Value<'c, 'a>,
        ptr: Value<'c, 'a>,
        location: Location<'c>,
    ) {
        let store_operation = llvm::store(self.context, value, ptr, location, Default::default());

        self.append_operation(current_block_id, block_references, store_operation);
    }

    fn gen_locals<'c, 'a>(
        &self,
        entry_block_id: usize,
        function_decl_id: &FunctionDeclarationID,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
    ) -> HashMap<Ssaid, Value<'c, 'a>>
    where
        'ctx: 'c,
    {
        let mut locals = HashMap::<Ssaid, Value<'c, 'a>>::new();
        let local_ir_variables = &self.program.ssa_variables[function_decl_id];
        let function_types = self.program_types.get(function_decl_id).unwrap();
        let entry_block = block_references[&entry_block_id];

        debug!("local types: {:#?}", function_types);

        for ssa_id in local_ir_variables.keys() {
            if function_types.comp_time_types.contains_key(ssa_id) {
                continue;
            }

            if function_types.comp_time_types.contains_key(ssa_id) {
                continue;
            }

            let fusion_type = if function_types.variable_types.contains_key(ssa_id) {
                &function_types
                    .lookup_variable_type(*ssa_id)
                    .unwrap_or_else(|_| panic!("failed to find type for: {:?}", ssa_id))
            } else {
                panic!("failed to find type for {:?}", ssa_id);
            };

            debug!("found type {:?} for ssa id {:?}", fusion_type, ssa_id);
            let inner_type = get_variable_mlir_type(
                self.context,
                self.program_types.get(function_decl_id).unwrap(),
                ssa_id,
            );

            let variable_ptr: Value = self
                .gen_allocate_on_stack_operation(
                    entry_block_id,
                    block_references,
                    inner_type,
                    Location::unknown(self.context),
                )
                .result(0)
                .unwrap()
                .into();

            locals.insert(*ssa_id, variable_ptr);

            // Fill out the value the projection points to.
            if function_types.is_projection(ssa_id) {
                let inner_type = as_mlir_type(*fusion_type, self.context, function_types);
                let variable_inner_mlir_value = self
                    .gen_allocate_on_stack_operation(
                        entry_block_id,
                        block_references,
                        inner_type,
                        Location::unknown(self.context),
                    )
                    .result(0)
                    .unwrap()
                    .into();

                self.gen_store_operation(
                    entry_block_id,
                    block_references,
                    variable_inner_mlir_value,
                    variable_ptr,
                    Location::unknown(self.context),
                );
            }

            let key = ssa_id;
            if self.program.static_values.contains_key(key) {
                let value = &self.program.static_values[key];
                let ptr = locals.get(key).unwrap();
                let ptr = if self.program_types[&self.current_fn_decl_id].is_projection(key) {
                    &self
                        .gen_variable_load(*key, block_references, &locals, entry_block_id)
                        .unwrap()
                } else {
                    ptr
                };

                match value {
                    nodes::Value::Integer(int) => {
                        let integer_val: Value = entry_block
                            .append_operation(melior::dialect::arith::constant(
                                self.context,
                                IntegerAttribute::new(inner_type, int.value as i64).into(),
                                Location::unknown(self.context),
                            ))
                            .result(0)
                            .unwrap()
                            .into();

                        let store_op = melior::dialect::llvm::store(
                            self.context,
                            integer_val,
                            *ptr,
                            melior::ir::Location::unknown(self.context),
                            Default::default(),
                        );

                        entry_block.append_operation(store_op);
                    }

                    nodes::Value::String(val) => {
                        // TODO: \n is getting escaped, perhap we need a raw string?
                        let val = if val == "\\n" { "\n" } else { val };
                        let val = val.replace("\\n", "\n");

                        let value: Value = self
                            .gen_pointer_to_annon_str(&entry_block, val.to_string())
                            .unwrap()
                            .result(0)
                            .unwrap()
                            .into();

                        let store_op = melior::dialect::llvm::store(
                            self.context,
                            value,
                            *ptr,
                            melior::ir::Location::unknown(self.context),
                            Default::default(),
                        );

                        entry_block.append_operation(store_op);

                        // MOVE: this to locals calculation
                        // variable_store.insert(*id, ptr_val.into());
                    }
                    _ => todo!("expected static value found {:?}", value),
                }
            }
        }

        locals
    }

    fn gen_function(
        &self,
        function_decl_id: &FunctionDeclarationID,
        cfg: &ControlFlowGraph<BlockId>,
        block_db: &BTreeMap<BlockId, ir::Block>,
    ) -> Result<Operation<'_>> {
        debug!("generating function: {function_decl_id:?}");

        let function_region = Region::new();
        let regions = Rc::new(RefCell::new(self.pre_gen_regions(cfg)?));

        let location = melior::ir::Location::unknown(self.context);
        let function_declaration = self
            .program
            .node_db
            .function_declarations
            .get(function_decl_id)
            .unwrap();

        let function_argument_types: Vec<Type> = if let Some(function_arguments) =
            self.program.function_arguments.get(function_decl_id)
        {
            function_arguments
                .iter()
                .map(|argument_id| {
                    get_variable_mlir_type(
                        self.context,
                        &self.program_types[function_decl_id],
                        argument_id,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        debug!(
            "creating function entry block with arguments: {}",
            function_argument_types.len()
        );
        let current_block = Block::new(
            function_argument_types
                .clone()
                .into_iter()
                .map(|arg_type| (arg_type, location))
                .collect::<Vec<(Type, Location)>>()
                .as_slice(),
        );

        let borrow_regions = regions.borrow();

        let result = self.pre_gen_blocks(cfg, &function_region, &borrow_regions, current_block)?;
        let local_variable_store = self.gen_locals(cfg.entry_point.0, function_decl_id, &result);

        // TODO: sort out how to handle entry block and fn args. use first block in region maybe?

        let mut ir_blocks = cfg.cycle_aware_successors(&cfg.entry_point)?;
        ir_blocks.insert(0, vec![cfg.entry_point]);

        debug!(
            "found ir blocks: {:?}, cfg: {:?}",
            ir_blocks, cfg.entry_point
        );

        for block_ids in ir_blocks {
            for block_id in block_ids {
                // Should only iterate over "top" level blocks, the rest are generated from inside other instructions.
                // Therefore check for blocks that dominates the entry block or is the entry block it self.
                // I don't thin the domination block is entirely correct yet.
                // Question: When do we need multiple top level blocks?
                debug!(
                    "block {:?} dominates entry {:?}",
                    block_id,
                    cfg.dominates3(block_id, cfg.entry_point)
                );
                let block = self.program.blocks.get(&block_id).unwrap();
                debug!(
                    "block {:?} create by control flow {:?}",
                    block_id, block.produced_directly_by_control_flow
                );
                if !block.produced_directly_by_control_flow
                    && (cfg.dominates3(block_id, cfg.entry_point) || block_id == cfg.entry_point)
                {
                    debug!("generating code for block {}", block_id);
                    self.gen_block(block_id, &result, &local_variable_store, block_db)?;
                }
            }
        }

        let function_region = function_region;
        let function_identifier = function_declaration.identifier.0.clone();
        let return_type = as_mlir_type(
            self.program_types[&self.program.entry_function_id]
                .lookup_type_name_type(&function_declaration.get_return_type())
                .unwrap(),
            self.context,
            &self.program_types[&self.program.entry_function_id],
        );

        let function_decl = if &function_identifier == "main" {
            func::func(
                self.context,
                StringAttribute::new(self.context, &function_identifier),
                TypeAttribute::new(
                    FunctionType::new(
                        self.context,
                        function_argument_types
                            .iter()
                            .map(|_| llvm::r#type::pointer(self.context, 0))
                            .collect::<Vec<Type>>()
                            .as_slice(),
                        &[],
                    )
                    .into(),
                ),
                function_region,
                &[(
                    Identifier::new(self.context, "llvm.emit_c_interface"),
                    Attribute::unit(self.context),
                )],
                location,
            )
        } else if function_declaration
            .keywords
            .contains(&FunctionKeyword::LlvmExtern)
        {
            llvm::func(
                self.context,
                StringAttribute::new(self.context, &function_identifier),
                TypeAttribute::new(llvm::r#type::function(
                    return_type,
                    function_argument_types.as_slice(),
                    false,
                )),
                function_region,
                &[
                    (
                        Identifier::new(self.context, "sym_visibility"),
                        StringAttribute::new(self.context, "private").into(),
                    ),
                    (
                        Identifier::new(self.context, "llvm.emit_c_interface"),
                        Attribute::unit(self.context),
                    ),
                ],
                location,
            )
        } else {
            let mlir_return_type = vec![return_type];

            func::func(
                self.context,
                StringAttribute::new(self.context, &function_identifier),
                TypeAttribute::new(
                    FunctionType::new(self.context, &function_argument_types, &mlir_return_type)
                        .into(),
                ),
                function_region,
                &[],
                location,
            )
        };

        Ok(function_decl)
    }

    pub fn gen_variable_load<'a, 'c>(
        &self,
        id: Ssaid,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
        current_block: usize,
    ) -> Result<Value<'c, 'a>>
    where
        'a: 'c,
        'ctx: 'a,
    {
        debug!("generating variabe load for {:?}", id);
        let value = variable_store.get(&id).unwrap().to_owned();

        debug!("found value {:?}", value);
        let current_block = block_references.get(&current_block).unwrap();
        let location = melior::ir::Location::unknown(self.context);

        let function_types = self.program_types.get(&self.current_fn_decl_id).unwrap();

        let fusion_type = if function_types.variable_types.contains_key(&id) {
            &function_types
                .lookup_variable_type(id)
                .unwrap_or_else(|_| panic!("failed to find type for: {:?}", id))
        } else {
            panic!("failed to find type for {:?}", id);
        };

        debug!("found type {:?} for ssa id {:?}", fusion_type, id);
        let inner_type = get_variable_mlir_type(
            self.context,
            self.program_types.get(&self.current_fn_decl_id).unwrap(),
            &id,
        );

        let result: Value = current_block
            .append_operation(llvm::load(
                self.context,
                value,
                inner_type,
                location,
                Default::default(),
            ))
            .result(0)?
            .into();

        debug!("found result {:?}", result);

        Ok(result)
    }

    fn gen_resultless_function_call<'a, 'c>(
        &self,
        function_id: FunctionId,
        arguments: Vec<Ssaid>,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
        current_block_id: usize,
    ) -> Result<()>
    where
        'a: 'c,
        'ctx: 'a,
    {
        let current_block = block_references.get(&current_block_id).unwrap();
        let argument_values = arguments
            .iter()
            .map(|arg_id| {
                self.gen_variable_load(*arg_id, block_references, variable_store, current_block_id)
                    .unwrap()
            })
            .collect::<Vec<Value>>();

        debug!("found function arguments: {:?}", argument_values);

        let function_declaration = self
            .program
            .node_db
            .function_declarations
            .get(&function_id.0)
            .unwrap();

        let return_type = &nodes::Type::Unit;
        let return_type = as_mlir_type(
            self.program_types[&self.program.entry_function_id]
                .lookup_type_name_type(return_type)
                .unwrap(),
            self.context,
            &self.program_types[&self.program.entry_function_id],
        );

        let location = melior::ir::Location::unknown(self.context);

        let call_operation = if function_declaration
            .keywords
            .contains(&FunctionKeyword::LlvmExtern)
        {
            debug!(
                "generating call operation for extern function {} with return type {:?}",
                function_declaration.identifier.0, return_type
            );
            OperationBuilder::new("func.call", location)
                .add_operands(&argument_values)
                .add_attributes(&[(
                    Identifier::new(self.context, "callee"),
                    FlatSymbolRefAttribute::new(self.context, &function_declaration.identifier.0)
                        .into(),
                )])
                .build()?
        } else {
            debug!(
                "generating call operation for internal function {} with return type {:?}",
                function_declaration.identifier.0, return_type
            );
            let return_types = { Vec::new() };

            func::call(
                self.context,
                FlatSymbolRefAttribute::new(self.context, &function_declaration.identifier.0),
                &argument_values,
                &return_types,
                location,
            )
        };

        if let Ok(_val) = current_block.append_operation(call_operation).result(0) {
            //let ptr_val = current_block.append_operation(ptr).result(0).unwrap();
            /*
            let ptr_val = variable_store[result_receiver];
            let store_op = melior::dialect::memref::store(
                val.into(),
                ptr_val.into(),
                &[],
                melior::ir::Location::unknown(self.context),
                );

            current_block.append_operation(store_op);
            */

            Ok(())
        } else {
            Ok(())
        }
    }

    // We don't make use of locations currently, so this is used as a sort of default value.
    // TODO: Is there something we could use the location for in an MLIR context?
    pub fn unknown_location(&self) -> Location<'_> {
        Location::unknown(self.context)
    }

    pub fn integer_attribute(&self, bit_width: u32, value: i64) -> IntegerAttribute<'_> {
        IntegerAttribute::new(IntegerType::new(self.context, bit_width).into(), value)
    }

    // See https://mlir.llvm.org/docs/Rationale/Rationale/#integer-signedness-semantics for a not
    // on unsigned integers in MLIR and LLVM.
    pub fn signless_integer_type(&self, bit_width: u32) -> IntegerType<'_> {
        melior::ir::r#type::IntegerType::new(self.context, bit_width)
    }

    pub fn opaque_pointer_type(&self) -> Type<'_> {
        // TODO: consider making this static somehow?
        llvm::r#type::pointer(self.context, 0)
    }

    pub fn load_variables<'c, 'a>(
        &self,
        variables: &[Ssaid],
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
        current_block: MlirBlockId,
    ) -> Result<Vec<Value<'c, 'a>>>
    where
        'ctx: 'c,
        'a: 'c,
    {
        variables
            .iter()
            .map(|arg_id| {
                self.gen_variable_load(*arg_id, block_references, variable_store, current_block.0)
            })
            .collect::<Result<Vec<Value>>>()
    }

    fn gen_function_call<'a, 'c>(
        &self,
        function_id: FunctionId,
        arguments: Vec<Ssaid>,
        block_references: &'a HashMap<usize, BlockRef<'c, 'a>>,
        variable_store: &HashMap<Ssaid, Value<'c, 'a>>,
        current_block_id: usize,
        result_receiver: &Ssaid,
    ) -> Result<()>
    where
        'a: 'c,
        'ctx: 'a,
    {
        let current_block = block_references.get(&current_block_id).unwrap();
        let argument_values = arguments
            .iter()
            .map(|arg_id| {
                self.gen_variable_load(*arg_id, block_references, variable_store, current_block_id)
                    .unwrap()
            })
            .collect::<Vec<Value>>();

        debug!("found function arguments: {:?}", argument_values);

        let function_declaration = self
            .program
            .node_db
            .function_declarations
            .get(&function_id.0)
            .unwrap();

        let original_return_type = function_declaration
            .return_type
            .as_ref()
            .unwrap_or(&nodes::Type::Unit);

        let return_type = as_mlir_type(
            self.program_types[&self.program.entry_function_id]
                .lookup_type_name_type(original_return_type)
                .unwrap(),
            self.context,
            &self.program_types[&self.program.entry_function_id],
        );

        let location = melior::ir::Location::unknown(self.context);

        let call_operation = if function_declaration
            .keywords
            .contains(&FunctionKeyword::LlvmExtern)
        {
            debug!(
                "generating call operation for extern function {} with return type {:?}",
                function_declaration.identifier.0, return_type
            );

            OperationBuilder::new("func.call", location)
                .add_operands(&argument_values)
                .add_attributes(&[(
                    Identifier::new(self.context, "callee"),
                    FlatSymbolRefAttribute::new(self.context, &function_declaration.identifier.0)
                        .into(),
                )])
                .add_results(&[return_type])
                .build()?
        } else {
            debug!(
                "generating call operation for internal function {} with return type {:?}",
                function_declaration.identifier.0, return_type
            );
            let return_types = if let nodes::Type::Unit = original_return_type {
                Vec::new()
            } else {
                vec![return_type]
            };

            func::call(
                self.context,
                FlatSymbolRefAttribute::new(self.context, &function_declaration.identifier.0),
                &argument_values,
                &return_types,
                location,
            )
        };

        if let Ok(val) = current_block.append_operation(call_operation).result(0) {
            debug!(
                "storing result {:?} for function {:?} into receiver {:?}",
                val, function_id, result_receiver
            );

            let ptr_val = variable_store[result_receiver];
            let store_op = melior::dialect::llvm::store(
                self.context,
                val.into(),
                ptr_val,
                melior::ir::Location::unknown(self.context),
                Default::default(),
            );

            current_block.append_operation(store_op);

            Ok(())
        } else {
            Ok(())
        }
    }

    fn pre_gen_regions(
        &self,
        cfg: &ControlFlowGraph<BlockId>,
    ) -> Result<HashMap<BlockId, Region<'_>>> {
        let mut regions = HashMap::<BlockId, Region<'_>>::new();

        for block_ids in cfg.cycle_aware_successors(&cfg.entry_point)? {
            for block_id in block_ids {
                let ir_block = self.program.blocks.get(&block_id).unwrap();

                // TODO: Not sure if this holds if we have multiple blocks in a conditional region.
                // Maybe we can default to parents region if the block is not a direct child of control flow?
                if ir_block.produced_directly_by_control_flow {
                    let region_for_block = Region::new();
                    regions.insert(block_id, region_for_block);
                }
            }
        }

        Ok(regions)
    }

    fn pre_gen_blocks<'a, 'c>(
        &self,
        cfg: &ControlFlowGraph<BlockId>,
        function_region: &'a Region<'c>,
        _regions: &'a Ref<'a, HashMap<BlockId, Region<'c>>>,
        entry_block: Block<'c>,
    ) -> Result<HashMap<usize, BlockRef<'c, 'a>>> {
        let entry_block_reference = function_region.append_block(entry_block);
        let mut result: HashMap<usize, BlockRef<'c, 'a>> = HashMap::new();
        result.insert(cfg.entry_point.0, entry_block_reference);

        for block_ids in cfg.cycle_aware_successors(&cfg.entry_point)? {
            for block_id in block_ids {
                let _block = Block::new(&[]);
                let ir_block = self.program.blocks.get(&block_id).unwrap();

                if !ir_block.produced_directly_by_control_flow {
                    result.insert(block_id.0, entry_block_reference);
                }
            }
        }

        Ok(result)
    }

    // NEXT: Make block generation pull based, using similar method to ir.rs. Where we return the next/current block allowing the generation code to decide what the next block should be.
    // This will allow generating blocks as needed for example in if/else cases.
    fn gen_block<'region, 'context, 'blocks, 'vars, 'varc>(
        &self,
        block_id: BlockId,
        block_references: &'blocks HashMap<usize, BlockRef<'context, 'region>>,
        variable_store: &'vars HashMap<Ssaid, Value<'varc, 'region>>,
        block_db: &BTreeMap<BlockId, ir::Block>,
    ) -> Result<()>
    where
        'ctx: 'context,
        'blocks: 'context,
        'vars: 'context,
        'region: 'varc,
    {
        let ir_block = block_db.get(&block_id).unwrap();
        // TODO: What about block arguments?

        for instruction in ir_block.instructions.iter() {
            self.gen_instruction(
                instruction,
                block_id.0,
                block_references,
                variable_store,
                block_db,
            )?;
        }

        Ok(())
    }

    fn generate_while_loop_operation<'parent_block, 'parent_context, 'context, 'this>(
        &self,
        condition_block_id: usize,
        body_block_id: usize,
        location: Location<'parent_context>,
        variable_store: &HashMap<Ssaid, Value<'parent_block, 'parent_block>>,
        block_db: &BTreeMap<BlockId, ir::Block>,
    ) -> Result<Operation<'context>>
    where
        'parent_context: 'this,
        'parent_block: 'this,
        'parent_context: 'context,
    {
        debug!(
            "generating while loop with blocks {:?}, {:?}",
            condition_block_id, body_block_id
        );
        let condition_block = Block::new(&[]);
        let body_block = Block::new(&[]);

        let condition_region = Region::new();
        let body_region = Region::new();

        let conditio_block_ref = condition_region.append_block(condition_block);
        let body_block_ref = body_region.append_block(body_block);

        let mut block_references: HashMap<usize, BlockRef<'_, '_>> = HashMap::new();
        block_references.insert(condition_block_id, conditio_block_ref);
        block_references.insert(body_block_id, body_block_ref);

        self.gen_block(
            BlockId(condition_block_id),
            &block_references,
            variable_store,
            block_db,
        )?;
        let condition_result = self.gen_variable_load(
            self.program
                .get_block_result(&BlockId(condition_block_id))
                .unwrap(),
            &block_references,
            variable_store,
            condition_block_id,
        )?;

        let condition_operation = scf::condition(condition_result, &[], location);
        conditio_block_ref.append_operation(condition_operation);

        // TODO: is there a scoping issue here? Could if block interefere with else block.
        self.gen_block(
            BlockId(body_block_id),
            &block_references,
            variable_store,
            block_db,
        )?;
        body_block_ref.append_operation(scf::r#yield(&[], location));

        Ok(melior::dialect::scf::r#while(
            &[],
            &[],
            condition_region,
            body_region,
            location,
        ))
    }

    fn generate_if_operation<'parent_block, 'parent_context, 'context, 'this>(
        &self,
        if_block_id: usize,
        condition: Value<'parent_context, 'parent_block>,
        location: Location<'parent_context>,
        variable_store: &HashMap<Ssaid, Value<'parent_block, 'parent_block>>,
        block_db: &BTreeMap<BlockId, ir::Block>,
    ) -> Result<Operation<'context>>
    where
        'parent_context: 'this,
        'parent_block: 'this,
        'parent_context: 'context,
    {
        debug!("generating if with block {:?}", if_block_id);
        let if_block = Block::new(&[]);

        let if_region = Region::new();

        let if_block_ref = if_region.append_block(if_block);

        let mut block_references: HashMap<usize, BlockRef<'_, '_>> = HashMap::new();
        block_references.insert(if_block_id, if_block_ref);

        self.gen_block(
            BlockId(if_block_id),
            &block_references,
            variable_store,
            block_db,
        )?;

        if_block_ref.append_operation(scf::r#yield(&[], location));

        Ok(melior::dialect::scf::r#if(
            condition,
            &[],
            if_region,
            Region::new(),
            location,
        ))
    }

    // TODO Next: Need to create a new mutable map of local vars for each operation layer and then provide a set of maps from parent scopes.
    fn generate_if_else_operation<'parent_block, 'parent_context, 'context, 'this>(
        &self,
        if_block_id: usize,
        else_block_id: usize,
        condition: Value<'parent_context, 'parent_block>,
        location: Location<'parent_context>,
        variable_store: &HashMap<Ssaid, Value<'parent_block, 'parent_block>>,
        block_db: &BTreeMap<BlockId, ir::Block>,
    ) -> Result<Operation<'context>>
    where
        'parent_context: 'this,
        'parent_block: 'this,
        'parent_context: 'context,
    {
        debug!(
            "generating if else with blocks {:?}, {:?}",
            if_block_id, else_block_id
        );
        let if_block = Block::new(&[]);
        let else_block = Block::new(&[]);

        let if_region = Region::new();
        let else_region = Region::new();

        let if_block_ref = if_region.append_block(if_block);
        let else_block_ref = else_region.append_block(else_block);

        let mut block_references: HashMap<usize, BlockRef<'_, '_>> = HashMap::new();
        block_references.insert(if_block_id, if_block_ref);
        block_references.insert(else_block_id, else_block_ref);

        self.gen_block(
            BlockId(if_block_id),
            &block_references,
            variable_store,
            block_db,
        )?;
        // TODO: is there a scoping issue here? Could if block interefere with else block.
        self.gen_block(
            BlockId(else_block_id),
            &block_references,
            variable_store,
            block_db,
        )?;

        if_block_ref.append_operation(scf::r#yield(&[], location));
        else_block_ref.append_operation(scf::r#yield(&[], location));

        Ok(melior::dialect::scf::r#if(
            condition,
            &[],
            if_region,
            else_region,
            location,
        ))
    }

    fn generate_arith_comparion<'context, 'region>(
        &self,
        operation_variables: ArithOperationVaribles,
        predicate: arith::CmpiPredicate,
        block_references: &HashMap<usize, BlockRef<'context, 'region>>,
        variable_store: &HashMap<Ssaid, Value<'context, 'region>>,
        current_block_id: usize,
    ) -> Result<()> {
        let current_block = block_references.get(&current_block_id).unwrap();
        let location = melior::ir::Location::unknown(self.context);
        let first_operand_value = self.gen_variable_load(
            operation_variables.left_hand_side,
            block_references,
            variable_store,
            current_block_id,
        )?;
        let second_operand_value = self.gen_variable_load(
            operation_variables.right_hand_side,
            block_references,
            variable_store,
            current_block_id,
        )?;
        let operation = melior::dialect::arith::cmpi(
            self.context,
            predicate,
            first_operand_value,
            second_operand_value,
            location,
        );

        let value = current_block.append_operation(operation).result(0)?;

        let ptr_val = variable_store[&operation_variables.reciever];
        let store_op = melior::dialect::llvm::store(
            self.context,
            value.into(),
            ptr_val,
            melior::ir::Location::unknown(self.context),
            Default::default(),
        );

        current_block.append_operation(store_op);

        Ok(())
    }

    fn gen_instruction<'parent_block, 'parent_context, 'context, 'this, 'blocks, 'vars, 'varc>(
        &self,
        instruction: &Instruction,
        current_block_id: usize,
        block_references: &'blocks HashMap<usize, BlockRef<'context, 'parent_block>>,
        variable_store: &'vars HashMap<Ssaid, Value<'varc, 'this>>,
        block_db: &BTreeMap<BlockId, ir::Block>,
    ) -> Result<Option<Value<'context, 'parent_block>>>
    where
        'ctx: 'this,
        'ctx: 'context,
        'this: 'context,
        'blocks: 'context,
        'blocks: 'vars,
        'vars: 'varc,
        'varc: 'context,
    {
        debug!(
            "generating instruction {:?} {:?}",
            instruction, block_references
        );
        let current_block = block_references.get(&current_block_id).unwrap();
        let location = melior::ir::Location::unknown(self.context);
        let result = match instruction {
            Instruction::AssignFnArg(id, position, _) => {
                debug!("declaring function argument {} {}", id.0, position);
                let value_ref = current_block.argument(*position).unwrap_or_else(|_| {
                    panic!(
                        "expected at least {} function arguments for fn",
                        position + 1
                    )
                });
                // MOVE NEXT: get fn args into variable store.
                let ptr = variable_store[id];

                current_block.append_operation(llvm::store(
                    self.context,
                    value_ref.into(),
                    ptr,
                    location,
                    Default::default(),
                ));

                // MOVE: move result reciever init to locals generation.
                // variable_store.insert(*id, ptr.into());
                None
            }
            Instruction::Project {
                reciever,
                projector,
            } => {
                if reciever != projector {
                    self.gen_projection(
                        reciever,
                        projector,
                        current_block_id,
                        block_references,
                        variable_store,
                    )?;
                }

                None
            }
            Instruction::StructAssign {
                r#struct,
                field_name_index,
                reciever,
            } => {
                self.struct_field_assignment(
                    r#struct,
                    reciever,
                    *field_name_index,
                    current_block_id,
                    block_references,
                    variable_store,
                )?;
                None
            }
            Instruction::AnnotatedAssign(AnnotatedAssignment {
                ref reciever,
                ref value,
                ..
            }) => {
                self.gen_assignment(
                    reciever,
                    value,
                    current_block_id,
                    block_references,
                    variable_store,
                )?;
                None
            }
            Instruction::Assign(ref lhs_id, ref rhs_id) => {
                self.gen_assignment(
                    lhs_id,
                    rhs_id,
                    current_block_id,
                    block_references,
                    variable_store,
                )?;
                None
            }
            Instruction::ResultlessCall(function_id, arguments) => {
                self.gen_resultless_function_call(
                    *function_id,
                    arguments.clone(),
                    block_references,
                    variable_store,
                    current_block_id,
                )?;

                None
            }
            Instruction::YieldingCall(function_id, arguments, result_reciever, _) => {
                self.gen_function_call(
                    *function_id,
                    arguments.clone(),
                    block_references,
                    variable_store,
                    current_block_id,
                    result_reciever,
                )?;

                variable_store.get(result_reciever).cloned()
            }
            Instruction::CallResultlessIntrinsic(intrinsic_call) => {
                generate_resultless_intrinsic_call(
                    intrinsic_call,
                    self,
                    MlirBlockId(current_block_id),
                    block_references,
                    variable_store,
                )?;

                None
            }
            Instruction::CallIntrinsic(intrinsic_call) => {
                generate_intrinsic_call(
                    intrinsic_call,
                    self,
                    MlirBlockId(current_block_id),
                    block_references,
                    variable_store,
                )?;

                variable_store.get(&intrinsic_call.result_receiver).cloned()
            }

            Instruction::Call(function_id, arguments, result_reciever, _) => {
                self.gen_function_call(
                    *function_id,
                    arguments.clone(),
                    block_references,
                    variable_store,
                    current_block_id,
                    result_reciever,
                )?;

                variable_store.get(result_reciever).cloned()
            }
            Instruction::Yield(result) => {
                let return_values = {
                    let val = self.gen_variable_load(
                        *result,
                        block_references,
                        variable_store,
                        current_block_id,
                    )?;

                    vec![val]
                };

                debug!("generating  instruction for block: {}", current_block_id);

                // Currently yield effectively operates as a return which extends the lifetime of the function parameters.
                current_block.append_operation(melior::dialect::func::r#return(
                    &return_values,
                    Location::unknown(self.context),
                ));
                None
            }
            Instruction::Return(result) => {
                let return_values = if let Some(expression) = result {
                    let val = self.gen_variable_load(
                        *expression,
                        block_references,
                        variable_store,
                        current_block_id,
                    )?;

                    vec![val]
                } else {
                    Vec::new()
                };

                debug!(
                    "generating return instruction for block: {}",
                    current_block_id
                );

                current_block.append_operation(melior::dialect::func::r#return(
                    &return_values,
                    Location::unknown(self.context),
                ));
                None
            }
            Instruction::Addition(lhs, rhs, result_reciever) => {
                let first_operand_value = self.gen_variable_load(
                    *lhs,
                    block_references,
                    variable_store,
                    current_block_id,
                )?;
                let second_operand_value = self.gen_variable_load(
                    *rhs,
                    block_references,
                    variable_store,
                    current_block_id,
                )?;
                let operation = melior::dialect::arith::addi(
                    first_operand_value,
                    second_operand_value,
                    location,
                );

                let value = current_block.append_operation(operation).result(0)?;

                let ptr_val = variable_store[result_reciever];

                let store_op = melior::dialect::llvm::store(
                    self.context,
                    value.into(),
                    ptr_val,
                    melior::ir::Location::unknown(self.context),
                    Default::default(),
                );

                current_block.append_operation(store_op);

                Some(ptr_val)
            }

            Instruction::GreaterThan(lhs, rhs, result_reciever) => {
                let operation_variables = ArithOperationVaribles {
                    left_hand_side: *lhs,
                    right_hand_side: *rhs,
                    reciever: *result_reciever,
                };

                self.generate_arith_comparion(
                    operation_variables,
                    arith::CmpiPredicate::Sgt,
                    block_references,
                    variable_store,
                    current_block_id,
                )?;
                let ptr_val = variable_store[result_reciever];
                Some(ptr_val)
            }

            Instruction::LessThan(lhs, rhs, result_reciever) => {
                let operation_variables = ArithOperationVaribles {
                    left_hand_side: *lhs,
                    right_hand_side: *rhs,
                    reciever: *result_reciever,
                };

                self.generate_arith_comparion(
                    operation_variables,
                    arith::CmpiPredicate::Slt,
                    block_references,
                    variable_store,
                    current_block_id,
                )?;
                let ptr_val = variable_store[result_reciever];
                Some(ptr_val)
            }
            Instruction::IfElse(condition, then_block, else_block) => {
                debug!("generate code for if else");
                let condition = self.gen_variable_load(
                    *condition,
                    block_references,
                    variable_store,
                    current_block_id,
                )?;

                // TODO: what is if/else is the last expression?

                let if_operation = self.generate_if_else_operation(
                    then_block.0,
                    else_block.0,
                    condition,
                    location,
                    variable_store,
                    block_db,
                );
                current_block.append_operation(if_operation?);

                None
            }
            Instruction::If(condition, then_block) => {
                debug!("generate code for if else");
                let condition = self.gen_variable_load(
                    *condition,
                    block_references,
                    variable_store,
                    current_block_id,
                )?;

                let if_operation = self.generate_if_operation(
                    then_block.0,
                    condition,
                    location,
                    variable_store,
                    block_db,
                );
                current_block.append_operation(if_operation?);

                None
            }
            Instruction::WhileLoop { condition, body } => {
                let while_operation = self.generate_while_loop_operation(
                    condition.0,
                    body.0,
                    location,
                    variable_store,
                    block_db,
                )?;

                current_block.append_operation(while_operation);
                None
            }
            Instruction::InitArray(items, result_receiver, _) => {
                let item_values = items
                    .iter()
                    .map(|item| {
                        self.gen_variable_load(
                            *item,
                            block_references,
                            variable_store,
                            current_block_id,
                        )
                    })
                    .collect::<Result<Vec<Value>>>()?;

                let Ok(mut mlir_array_value) = self.gen_variable_load(
                    *result_receiver,
                    block_references,
                    variable_store,
                    current_block_id,
                ) else {
                    bail!("failed to find struct {}", result_receiver.0);
                };

                for (index, item) in item_values.into_iter().enumerate() {
                    mlir_array_value = current_block
                        .append_operation(llvm::insert_value(
                            self.context,
                            mlir_array_value,
                            DenseI64ArrayAttribute::new(self.context, &[index as i64]),
                            item,
                            location,
                        ))
                        .result(0)
                        .unwrap()
                        .into();
                }

                let ptr_val: Value = variable_store[result_receiver];

                let store_op = melior::dialect::llvm::store(
                    self.context,
                    mlir_array_value,
                    ptr_val,
                    melior::ir::Location::unknown(self.context),
                    Default::default(),
                );
                current_block.append_operation(store_op);

                Some(ptr_val)
            }
            Instruction::StructInit {
                field_values,
                receiver,
                ..
            } => {
                let field_value_ptrs: Vec<Value> = field_values
                    .iter()
                    .map(|field_value_id| {
                        self.gen_variable_load(
                            *field_value_id,
                            block_references,
                            variable_store,
                            current_block_id,
                        )
                    })
                    .collect::<Result<Vec<Value>>>()?;

                let Ok(mut mlir_struct_value) = self.gen_variable_load(
                    *receiver,
                    block_references,
                    variable_store,
                    current_block_id,
                ) else {
                    bail!("failed to find struct {}", receiver.0);
                };

                for (i, field_value_ptr) in field_value_ptrs.iter().enumerate() {
                    let insertion_instruction = llvm::insert_value(
                        self.context,
                        mlir_struct_value,
                        melior::ir::attribute::DenseI64ArrayAttribute::new(
                            self.context,
                            &[i as i64],
                        ),
                        *field_value_ptr,
                        location,
                    );

                    mlir_struct_value = current_block
                        .append_operation(insertion_instruction)
                        .result(0)
                        .unwrap()
                        .into();
                }

                let mlir_struct_ref = variable_store[receiver];

                let store_op = melior::dialect::llvm::store(
                    self.context,
                    mlir_struct_value,
                    mlir_struct_ref,
                    melior::ir::Location::unknown(self.context),
                    Default::default(),
                );
                current_block.append_operation(store_op);

                Some(mlir_struct_ref)
            }
            Instruction::ReadStructField {
                r#struct,
                field,
                receiver,
            } => {
                let program_types = self.program_types.get(&self.current_fn_decl_id).unwrap();
                let field_index = program_types.calculate_struct_field_position(
                    *r#struct,
                    *field,
                    &self.program,
                )?;

                let Ok(struct_ptr) = self.gen_variable_load(
                    *r#struct,
                    block_references,
                    variable_store,
                    current_block_id,
                ) else {
                    bail!("failed to find struct {}", r#struct.0);
                };

                let struct_ptr =
                    if self.program_types[&self.current_fn_decl_id].is_projection(r#struct) {
                        let field_type = program_types.lookup_variable_type(*r#struct)?;
                        let field_mlir_type = as_mlir_type(field_type, self.context, program_types);

                        let result: Value = self
                            .gen_load_from_stack_operation(
                                current_block_id,
                                block_references,
                                struct_ptr,
                                field_mlir_type,
                                location,
                            )
                            .result(0)?
                            .into();

                        result
                    } else {
                        struct_ptr
                    };

                let reciver_val = variable_store[receiver];
                let field_type = program_types.lookup_variable_type(*receiver)?;
                let field_mlir_type = as_mlir_type(field_type, self.context, program_types);

                let read_instruction = llvm::extract_value(
                    self.context,
                    struct_ptr,
                    melior::ir::attribute::DenseI64ArrayAttribute::new(
                        self.context,
                        &[field_index as i64],
                    ),
                    field_mlir_type,
                    location,
                );

                let read_value: Value = current_block
                    .append_operation(read_instruction)
                    .result(0)
                    .unwrap()
                    .into();
                let store_op = llvm::store(
                    self.context,
                    read_value,
                    reciver_val,
                    location,
                    Default::default(),
                );
                current_block.append_operation(store_op);

                Some(read_value)
            }
            Instruction::ArrayLookup {
                array,
                index,
                result,
            } => {
                let Ok(index_ptr) = self.gen_variable_load(
                    *index,
                    block_references,
                    variable_store,
                    current_block_id,
                ) else {
                    bail!("failed to find index {}", index.0);
                };
                let Some(array_ptr) = variable_store.get(array) else {
                    bail!("failed to find array {}", array.0);
                };

                let program_types = self.program_types.get(&self.current_fn_decl_id).unwrap();
                let array_type = program_types.lookup_variable_type(*array)?;
                let element_type = match array_type {
                    types::Type::Array(array_type_id) => {
                        let array_type = program_types.array_types.get(array_type_id).unwrap();
                        let fusion_element_type = program_types
                            .comp_time_types
                            .get(&array_type.element_type)
                            .unwrap();
                        as_mlir_type(*fusion_element_type, self.context, program_types)
                    }
                    _ => panic!(),
                };

                let ptr_type = melior::dialect::llvm::r#type::pointer(self.context, 0);

                let gep_op_2: Value = current_block
                    .append_operation(llvm::get_element_ptr_dynamic(
                        self.context,
                        *array_ptr,
                        &[index_ptr],
                        element_type,
                        ptr_type,
                        location,
                    ))
                    .result(0)
                    .unwrap()
                    .into();

                let load_element_val = current_block
                    .append_operation(llvm::load(
                        self.context,
                        gep_op_2,
                        element_type,
                        location,
                        Default::default(),
                    ))
                    .result(0)
                    .unwrap();

                let ptr_val: Value = variable_store[result];

                let store_op = melior::dialect::llvm::store(
                    self.context,
                    load_element_val.into(),
                    ptr_val,
                    melior::ir::Location::unknown(self.context),
                    Default::default(),
                );
                current_block.append_operation(store_op);

                Some(ptr_val)
            }
            Instruction::AnonymousValue(_) => None,
            Instruction::MutBorrow(_)
            | Instruction::MutBorrowEnd(_)
            | Instruction::BorrowEnd(_)
            | Instruction::Move(_)
            | Instruction::Borrow(_)
            | Instruction::Drop(_) => None,
            _ => panic!("instruction not implemented yet {:?}", instruction),
        };

        Ok(result)
    }

    pub fn gen_pointer_to_annon_str<'a, 'c>(
        &self,
        current_block: &'a BlockRef<'c, 'a>,
        value: String,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'ctx: 'a,
        'ctx: 'c,
    {
        self.generate_annon_string(value, current_block)
    }
    fn gen_annon_string_id(&self) -> String {
        let id = format!("annonstr{}", self.annon_string_counter.borrow());

        self.annon_string_counter.replace_with(|&mut v| v + 1_usize);

        id
    }

    fn generate_annon_string<'a, 'c>(
        &self,
        value: String,
        current_block: &'a Block<'c>,
    ) -> Result<OperationRef<'c, 'a>>
    where
        'ctx: 'a,
        'ctx: 'c,
    {
        let location = melior::ir::Location::unknown(self.context);
        let id = self.gen_annon_string_id();
        let string_type = llvm::r#type::array(
            IntegerType::new(self.context, 8).into(),
            (value.len()) as u32,
        );
        let op = OperationBuilder::new("llvm.mlir.global", location)
            .add_regions([Region::new()])
            .add_attributes(&[
                (
                    Identifier::new(self.context, "value"),
                    StringAttribute::new(self.context, &value.to_string()).into(),
                ),
                (
                    Identifier::new(self.context, "sym_name"),
                    StringAttribute::new(self.context, &id).into(),
                ),
                (
                    Identifier::new(self.context, "global_type"),
                    TypeAttribute::new(string_type).into(),
                ),
                (
                    Identifier::new(self.context, "linkage"),
                    llvm::attributes::linkage(self.context, Linkage::Internal),
                ),
            ])
            .build()?;

        self.module.body().append_operation(op);

        let address_op = OperationBuilder::new("llvm.mlir.addressof", location)
            // .enable_result_type_inference()
            .add_attributes(&[(
                Identifier::new(self.context, "global_name"),
                FlatSymbolRefAttribute::new(self.context, &id).into(),
            )])
            .add_results(&[llvm::r#type::pointer(self.context, 0)])
            .build()?;

        Ok(current_block.append_operation(address_op))
    }

    fn gen_projection<'parent_block, 'context, 'varc, 'this>(
        &self,
        lhs_id: &Ssaid,
        rhs_id: &Ssaid,
        current_block: usize,
        block_references: &'this HashMap<usize, BlockRef<'context, 'parent_block>>,
        variable_store: &'this HashMap<Ssaid, Value<'varc, 'parent_block>>,
    ) -> Result<()>
    where
        'ctx: 'context,
        'this: 'context,
    {
        let lhs_is_projection = self.program_types[&self.current_fn_decl_id].is_projection(lhs_id);
        let rhs_is_projection = self.program_types[&self.current_fn_decl_id].is_projection(rhs_id);
        assert!(lhs_is_projection);

        debug!("generating assignment {:?} {:?}", lhs_id, rhs_id);
        let rhs_value = if rhs_is_projection {
            self.gen_variable_load(*rhs_id, block_references, variable_store, current_block)?
        } else {
            variable_store.get(rhs_id).unwrap().to_owned()
        };
        let lhs_ptr = variable_store.get(lhs_id).unwrap().to_owned();

        self.gen_store_operation(
            current_block,
            block_references,
            rhs_value,
            lhs_ptr,
            melior::ir::Location::unknown(self.context),
        );

        Ok(())
    }

    fn struct_field_assignment<'parent_block, 'context, 'varc, 'this>(
        &self,
        lhs_id: &Ssaid,
        rhs_id: &Ssaid,
        field_name_index: usize,
        current_block_id: usize,
        block_references: &'this HashMap<usize, BlockRef<'context, 'parent_block>>,
        variable_store: &'this HashMap<Ssaid, Value<'varc, 'parent_block>>,
    ) -> Result<()>
    where
        'ctx: 'context,
        'this: 'context,
    {
        debug!("generating assignment {:?} {:?}", lhs_id, rhs_id);
        let program_types = self.program_types.get(&self.current_fn_decl_id).unwrap();
        let struct_type = program_types.lookup_variable_type(*lhs_id)?;
        let struct_mlir_type = as_mlir_type(struct_type, self.context, program_types);
        let rhs_value =
            self.gen_variable_load(*rhs_id, block_references, variable_store, current_block_id)?;
        let lhs_ptr = if self.program_types[&self.current_fn_decl_id].is_projection(lhs_id) {
            let struct_ptr = self.gen_variable_load(
                *lhs_id,
                block_references,
                variable_store,
                current_block_id,
            )?;

            let result: Value = self
                .gen_load_from_stack_operation(
                    current_block_id,
                    block_references,
                    struct_ptr,
                    struct_mlir_type,
                    melior::ir::Location::unknown(self.context),
                )
                .result(0)?
                .into();

            result
        } else {
            self.gen_variable_load(*lhs_id, block_references, variable_store, current_block_id)?
        };

        let field_position = self.program_types[&self.current_fn_decl_id]
            .calculate_struct_field_position(*lhs_id, field_name_index, &self.program)?;

        let store_op = llvm::insert_value(
            self.context,
            lhs_ptr,
            melior::ir::attribute::DenseI64ArrayAttribute::new(
                self.context,
                &[field_position as i64],
            ),
            rhs_value,
            melior::ir::Location::unknown(self.context),
        );

        let lhs_ptr = if self.program_types[&self.current_fn_decl_id].is_projection(lhs_id) {
            self.gen_variable_load(*lhs_id, block_references, variable_store, current_block_id)?
        } else {
            variable_store.get(lhs_id).unwrap().to_owned()
        };

        let current_block = block_references.get(&current_block_id).unwrap();
        // TODO: we are assigning the entire struct here: just write to the field location instead.
        let new_struct_val: Value = current_block
            .append_operation(store_op)
            .result(0)
            .unwrap()
            .into();
        self.gen_store_operation(
            current_block_id,
            block_references,
            new_struct_val,
            lhs_ptr,
            melior::ir::Location::unknown(self.context),
        );

        Ok(())
    }

    fn gen_assignment<'parent_block, 'context, 'varc, 'this>(
        &self,
        lhs_id: &Ssaid,
        rhs_id: &Ssaid,
        current_block: usize,
        block_references: &'this HashMap<usize, BlockRef<'context, 'parent_block>>,
        variable_store: &'this HashMap<Ssaid, Value<'varc, 'parent_block>>,
    ) -> Result<()>
    where
        'ctx: 'context,
        'this: 'context,
    {
        debug!("generating assignment {:?} {:?}", lhs_id, rhs_id);

        let lhs_is_projection = self.program_types[&self.current_fn_decl_id].is_projection(lhs_id);
        let rhs_is_projection = self.program_types[&self.current_fn_decl_id].is_projection(rhs_id);
        assert!(lhs_is_projection == rhs_is_projection);

        let rhs_value =
            self.gen_variable_load(*rhs_id, block_references, variable_store, current_block)?;

        let lhs_ptr = variable_store.get(lhs_id).unwrap().to_owned();

        let store_op = melior::dialect::llvm::store(
            self.context,
            rhs_value,
            lhs_ptr,
            melior::ir::Location::unknown(self.context),
            Default::default(),
        );

        let current_block = block_references.get(&current_block).unwrap();

        current_block.append_operation(store_op);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use anyhow::Result;
    use rstest::rstest;

    use super::*;

    fn generate_mlir_string(cfg: MlirGenerationConfig) -> Result<String> {
        let context = prepare_mlir_context();
        let mut module = Module::new(melior::ir::Location::unknown(&context));
        let mut code_gen = Box::new(CodeGen::new(
            &context,
            &module,
            cfg.program,
            cfg.program_types,
        ));
        code_gen.gen_code()?;
        run_mlir_passes(&context, &mut module);

        if cfg.verify_mlir {
            assert!(module.as_operation().verify());
        }
        Ok(format!("{}", module.as_operation()))
    }

    #[rstest]
    #[test_log::test]
    fn test_ir_output(#[files("./ir_test_programs/test_*.ts")] path: PathBuf) -> Result<()> {
        use crate::analysis::type_evaluation::evaluate_types;
        use crate::compiler::produce_ir;

        let ir_program = produce_ir(path.to_str().unwrap())?;
        debug!(
            "testing codegen for IR: {ir_program} \n cfg: {:?}",
            ir_program.control_flow_graphs
        );
        let ir_types = evaluate_types(&ir_program)?;
        let mlir_generation_config = MlirGenerationConfig {
            verify_mlir: true,
            program: ir_program.clone(),
            program_types: ir_types,
        };

        let mlir_output = generate_mlir_string(mlir_generation_config)?;

        insta::assert_snapshot!(
            format!(
                "test_well_formed_ir_{}",
                path.file_name().unwrap().to_str().unwrap()
            ),
            format!("{}", mlir_output)
        );

        Ok(())
    }
}
