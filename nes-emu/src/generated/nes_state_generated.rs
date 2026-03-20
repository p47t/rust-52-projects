pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.3.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `d:\code\rust\rust-52-projects\nes-emu\schemas\nes_state.fbs`
#[no_implicit_prelude]
#[allow(clippy::needless_lifetimes)]
mod root {
    /// The namespace `nes_state`
    ///
    /// Generated from these locations:
    /// * File `d:\code\rust\rust-52-projects\nes-emu\schemas\nes_state.fbs`
    pub mod nes_state {
        /// The table `CpuState` in the namespace `nes_state`
        ///
        /// Generated from these locations:
        /// * Table `CpuState` in the file `d:\code\rust\rust-52-projects\nes-emu\schemas\nes_state.fbs:8`
        #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
        pub struct CpuState {
            /// The field `a` in the table `CpuState`
            pub a: u8,
            /// The field `x` in the table `CpuState`
            pub x: u8,
            /// The field `y` in the table `CpuState`
            pub y: u8,
            /// The field `pc` in the table `CpuState`
            pub pc: u16,
            /// The field `sp` in the table `CpuState`
            pub sp: u8,
            /// The field `p` in the table `CpuState`
            pub p: u8,
            /// The field `cycles` in the table `CpuState`
            pub cycles: u64,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for CpuState {
            fn default() -> Self {
                Self {
                    a: 0,
                    x: 0,
                    y: 0,
                    pc: 0,
                    sp: 0,
                    p: 0,
                    cycles: 0,
                }
            }
        }

        impl CpuState {
            /// Creates a [CpuStateBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> CpuStateBuilder<()> {
                CpuStateBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_a: impl ::planus::WriteAsDefault<u8, u8>,
                field_x: impl ::planus::WriteAsDefault<u8, u8>,
                field_y: impl ::planus::WriteAsDefault<u8, u8>,
                field_pc: impl ::planus::WriteAsDefault<u16, u16>,
                field_sp: impl ::planus::WriteAsDefault<u8, u8>,
                field_p: impl ::planus::WriteAsDefault<u8, u8>,
                field_cycles: impl ::planus::WriteAsDefault<u64, u64>,
            ) -> ::planus::Offset<Self> {
                let prepared_a = field_a.prepare(builder, &0);
                let prepared_x = field_x.prepare(builder, &0);
                let prepared_y = field_y.prepare(builder, &0);
                let prepared_pc = field_pc.prepare(builder, &0);
                let prepared_sp = field_sp.prepare(builder, &0);
                let prepared_p = field_p.prepare(builder, &0);
                let prepared_cycles = field_cycles.prepare(builder, &0);

                let mut table_writer: ::planus::table_writer::TableWriter<18> =
                    ::core::default::Default::default();
                if prepared_cycles.is_some() {
                    table_writer.write_entry::<u64>(6);
                }
                if prepared_pc.is_some() {
                    table_writer.write_entry::<u16>(3);
                }
                if prepared_a.is_some() {
                    table_writer.write_entry::<u8>(0);
                }
                if prepared_x.is_some() {
                    table_writer.write_entry::<u8>(1);
                }
                if prepared_y.is_some() {
                    table_writer.write_entry::<u8>(2);
                }
                if prepared_sp.is_some() {
                    table_writer.write_entry::<u8>(4);
                }
                if prepared_p.is_some() {
                    table_writer.write_entry::<u8>(5);
                }

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        if let ::core::option::Option::Some(prepared_cycles) = prepared_cycles {
                            object_writer.write::<_, _, 8>(&prepared_cycles);
                        }
                        if let ::core::option::Option::Some(prepared_pc) = prepared_pc {
                            object_writer.write::<_, _, 2>(&prepared_pc);
                        }
                        if let ::core::option::Option::Some(prepared_a) = prepared_a {
                            object_writer.write::<_, _, 1>(&prepared_a);
                        }
                        if let ::core::option::Option::Some(prepared_x) = prepared_x {
                            object_writer.write::<_, _, 1>(&prepared_x);
                        }
                        if let ::core::option::Option::Some(prepared_y) = prepared_y {
                            object_writer.write::<_, _, 1>(&prepared_y);
                        }
                        if let ::core::option::Option::Some(prepared_sp) = prepared_sp {
                            object_writer.write::<_, _, 1>(&prepared_sp);
                        }
                        if let ::core::option::Option::Some(prepared_p) = prepared_p {
                            object_writer.write::<_, _, 1>(&prepared_p);
                        }
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<CpuState>> for CpuState {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CpuState> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<CpuState>> for CpuState {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<CpuState>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<CpuState> for CpuState {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CpuState> {
                CpuState::create(
                    builder,
                    self.a,
                    self.x,
                    self.y,
                    self.pc,
                    self.sp,
                    self.p,
                    self.cycles,
                )
            }
        }

        /// Builder for serializing an instance of the [CpuState] type.
        ///
        /// Can be created using the [CpuState::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct CpuStateBuilder<State>(State);

        impl CpuStateBuilder<()> {
            /// Setter for the [`a` field](CpuState#structfield.a).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn a<T0>(self, value: T0) -> CpuStateBuilder<(T0,)>
            where
                T0: ::planus::WriteAsDefault<u8, u8>,
            {
                CpuStateBuilder((value,))
            }

            /// Sets the [`a` field](CpuState#structfield.a) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn a_as_default(self) -> CpuStateBuilder<(::planus::DefaultValue,)> {
                self.a(::planus::DefaultValue)
            }
        }

        impl<T0> CpuStateBuilder<(T0,)> {
            /// Setter for the [`x` field](CpuState#structfield.x).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn x<T1>(self, value: T1) -> CpuStateBuilder<(T0, T1)>
            where
                T1: ::planus::WriteAsDefault<u8, u8>,
            {
                let (v0,) = self.0;
                CpuStateBuilder((v0, value))
            }

            /// Sets the [`x` field](CpuState#structfield.x) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn x_as_default(self) -> CpuStateBuilder<(T0, ::planus::DefaultValue)> {
                self.x(::planus::DefaultValue)
            }
        }

        impl<T0, T1> CpuStateBuilder<(T0, T1)> {
            /// Setter for the [`y` field](CpuState#structfield.y).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn y<T2>(self, value: T2) -> CpuStateBuilder<(T0, T1, T2)>
            where
                T2: ::planus::WriteAsDefault<u8, u8>,
            {
                let (v0, v1) = self.0;
                CpuStateBuilder((v0, v1, value))
            }

            /// Sets the [`y` field](CpuState#structfield.y) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn y_as_default(self) -> CpuStateBuilder<(T0, T1, ::planus::DefaultValue)> {
                self.y(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2> CpuStateBuilder<(T0, T1, T2)> {
            /// Setter for the [`pc` field](CpuState#structfield.pc).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn pc<T3>(self, value: T3) -> CpuStateBuilder<(T0, T1, T2, T3)>
            where
                T3: ::planus::WriteAsDefault<u16, u16>,
            {
                let (v0, v1, v2) = self.0;
                CpuStateBuilder((v0, v1, v2, value))
            }

            /// Sets the [`pc` field](CpuState#structfield.pc) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn pc_as_default(self) -> CpuStateBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                self.pc(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3> CpuStateBuilder<(T0, T1, T2, T3)> {
            /// Setter for the [`sp` field](CpuState#structfield.sp).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn sp<T4>(self, value: T4) -> CpuStateBuilder<(T0, T1, T2, T3, T4)>
            where
                T4: ::planus::WriteAsDefault<u8, u8>,
            {
                let (v0, v1, v2, v3) = self.0;
                CpuStateBuilder((v0, v1, v2, v3, value))
            }

            /// Sets the [`sp` field](CpuState#structfield.sp) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn sp_as_default(
                self,
            ) -> CpuStateBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                self.sp(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4> CpuStateBuilder<(T0, T1, T2, T3, T4)> {
            /// Setter for the [`p` field](CpuState#structfield.p).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn p<T5>(self, value: T5) -> CpuStateBuilder<(T0, T1, T2, T3, T4, T5)>
            where
                T5: ::planus::WriteAsDefault<u8, u8>,
            {
                let (v0, v1, v2, v3, v4) = self.0;
                CpuStateBuilder((v0, v1, v2, v3, v4, value))
            }

            /// Sets the [`p` field](CpuState#structfield.p) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn p_as_default(
                self,
            ) -> CpuStateBuilder<(T0, T1, T2, T3, T4, ::planus::DefaultValue)> {
                self.p(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5> CpuStateBuilder<(T0, T1, T2, T3, T4, T5)> {
            /// Setter for the [`cycles` field](CpuState#structfield.cycles).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn cycles<T6>(self, value: T6) -> CpuStateBuilder<(T0, T1, T2, T3, T4, T5, T6)>
            where
                T6: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2, v3, v4, v5) = self.0;
                CpuStateBuilder((v0, v1, v2, v3, v4, v5, value))
            }

            /// Sets the [`cycles` field](CpuState#structfield.cycles) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn cycles_as_default(
                self,
            ) -> CpuStateBuilder<(T0, T1, T2, T3, T4, T5, ::planus::DefaultValue)> {
                self.cycles(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6> CpuStateBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CpuState].
            #[inline]
            pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<CpuState>
            where
                Self: ::planus::WriteAsOffset<CpuState>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsDefault<u8, u8>,
                T2: ::planus::WriteAsDefault<u8, u8>,
                T3: ::planus::WriteAsDefault<u16, u16>,
                T4: ::planus::WriteAsDefault<u8, u8>,
                T5: ::planus::WriteAsDefault<u8, u8>,
                T6: ::planus::WriteAsDefault<u64, u64>,
            > ::planus::WriteAs<::planus::Offset<CpuState>>
            for CpuStateBuilder<(T0, T1, T2, T3, T4, T5, T6)>
        {
            type Prepared = ::planus::Offset<CpuState>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CpuState> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsDefault<u8, u8>,
                T2: ::planus::WriteAsDefault<u8, u8>,
                T3: ::planus::WriteAsDefault<u16, u16>,
                T4: ::planus::WriteAsDefault<u8, u8>,
                T5: ::planus::WriteAsDefault<u8, u8>,
                T6: ::planus::WriteAsDefault<u64, u64>,
            > ::planus::WriteAsOptional<::planus::Offset<CpuState>>
            for CpuStateBuilder<(T0, T1, T2, T3, T4, T5, T6)>
        {
            type Prepared = ::planus::Offset<CpuState>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<CpuState>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsDefault<u8, u8>,
                T2: ::planus::WriteAsDefault<u8, u8>,
                T3: ::planus::WriteAsDefault<u16, u16>,
                T4: ::planus::WriteAsDefault<u8, u8>,
                T5: ::planus::WriteAsDefault<u8, u8>,
                T6: ::planus::WriteAsDefault<u64, u64>,
            > ::planus::WriteAsOffset<CpuState> for CpuStateBuilder<(T0, T1, T2, T3, T4, T5, T6)>
        {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CpuState> {
                let (v0, v1, v2, v3, v4, v5, v6) = &self.0;
                CpuState::create(builder, v0, v1, v2, v3, v4, v5, v6)
            }
        }

        /// Reference to a deserialized [CpuState].
        #[derive(Copy, Clone)]
        pub struct CpuStateRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

        impl<'a> CpuStateRef<'a> {
            /// Getter for the [`a` field](CpuState#structfield.a).
            #[inline]
            pub fn a(&self) -> ::planus::Result<u8> {
                ::core::result::Result::Ok(self.0.access(0, "CpuState", "a")?.unwrap_or(0))
            }

            /// Getter for the [`x` field](CpuState#structfield.x).
            #[inline]
            pub fn x(&self) -> ::planus::Result<u8> {
                ::core::result::Result::Ok(self.0.access(1, "CpuState", "x")?.unwrap_or(0))
            }

            /// Getter for the [`y` field](CpuState#structfield.y).
            #[inline]
            pub fn y(&self) -> ::planus::Result<u8> {
                ::core::result::Result::Ok(self.0.access(2, "CpuState", "y")?.unwrap_or(0))
            }

            /// Getter for the [`pc` field](CpuState#structfield.pc).
            #[inline]
            pub fn pc(&self) -> ::planus::Result<u16> {
                ::core::result::Result::Ok(self.0.access(3, "CpuState", "pc")?.unwrap_or(0))
            }

            /// Getter for the [`sp` field](CpuState#structfield.sp).
            #[inline]
            pub fn sp(&self) -> ::planus::Result<u8> {
                ::core::result::Result::Ok(self.0.access(4, "CpuState", "sp")?.unwrap_or(0))
            }

            /// Getter for the [`p` field](CpuState#structfield.p).
            #[inline]
            pub fn p(&self) -> ::planus::Result<u8> {
                ::core::result::Result::Ok(self.0.access(5, "CpuState", "p")?.unwrap_or(0))
            }

            /// Getter for the [`cycles` field](CpuState#structfield.cycles).
            #[inline]
            pub fn cycles(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(self.0.access(6, "CpuState", "cycles")?.unwrap_or(0))
            }
        }

        impl<'a> ::core::fmt::Debug for CpuStateRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("CpuStateRef");
                f.field("a", &self.a());
                f.field("x", &self.x());
                f.field("y", &self.y());
                f.field("pc", &self.pc());
                f.field("sp", &self.sp());
                f.field("p", &self.p());
                f.field("cycles", &self.cycles());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<CpuStateRef<'a>> for CpuState {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: CpuStateRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    a: ::core::convert::TryInto::try_into(value.a()?)?,
                    x: ::core::convert::TryInto::try_into(value.x()?)?,
                    y: ::core::convert::TryInto::try_into(value.y()?)?,
                    pc: ::core::convert::TryInto::try_into(value.pc()?)?,
                    sp: ::core::convert::TryInto::try_into(value.sp()?)?,
                    p: ::core::convert::TryInto::try_into(value.p()?)?,
                    cycles: ::core::convert::TryInto::try_into(value.cycles()?)?,
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for CpuStateRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for CpuStateRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location("[CpuStateRef]", "get", buffer.offset_from_start)
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<CpuState>> for CpuState {
            type Value = ::planus::Offset<CpuState>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<CpuState>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for CpuStateRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[CpuStateRef]", "read_as_root", 0)
                })
            }
        }

        /// The table `NesState` in the namespace `nes_state`
        ///
        /// Generated from these locations:
        /// * Table `NesState` in the file `d:\code\rust\rust-52-projects\nes-emu\schemas\nes_state.fbs:18`
        #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
        pub struct NesState {
            /// The field `version` in the table `NesState`
            pub version: u8,
            /// The field `cpu` in the table `NesState`
            pub cpu: ::core::option::Option<::planus::alloc::boxed::Box<self::CpuState>>,
            /// The field `ram` in the table `NesState`
            pub ram: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            /// The field `mapper` in the table `NesState`
            pub mapper: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            /// The field `ppu` in the table `NesState`
            pub ppu: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            /// The field `apu` in the table `NesState`
            pub apu: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            /// The field `joypad1` in the table `NesState`
            pub joypad1: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            /// The field `joypad2` in the table `NesState`
            pub joypad2: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            /// The field `cpu_cycles` in the table `NesState`
            pub cpu_cycles: u64,
            /// The field `ppu_cycles` in the table `NesState`
            pub ppu_cycles: u64,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for NesState {
            fn default() -> Self {
                Self {
                    version: 2,
                    cpu: ::core::default::Default::default(),
                    ram: ::core::default::Default::default(),
                    mapper: ::core::default::Default::default(),
                    ppu: ::core::default::Default::default(),
                    apu: ::core::default::Default::default(),
                    joypad1: ::core::default::Default::default(),
                    joypad2: ::core::default::Default::default(),
                    cpu_cycles: 0,
                    ppu_cycles: 0,
                }
            }
        }

        impl NesState {
            /// Creates a [NesStateBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> NesStateBuilder<()> {
                NesStateBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_version: impl ::planus::WriteAsDefault<u8, u8>,
                field_cpu: impl ::planus::WriteAsOptional<::planus::Offset<self::CpuState>>,
                field_ram: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                field_mapper: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                field_ppu: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                field_apu: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                field_joypad1: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                field_joypad2: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                field_cpu_cycles: impl ::planus::WriteAsDefault<u64, u64>,
                field_ppu_cycles: impl ::planus::WriteAsDefault<u64, u64>,
            ) -> ::planus::Offset<Self> {
                let prepared_version = field_version.prepare(builder, &2);
                let prepared_cpu = field_cpu.prepare(builder);
                let prepared_ram = field_ram.prepare(builder);
                let prepared_mapper = field_mapper.prepare(builder);
                let prepared_ppu = field_ppu.prepare(builder);
                let prepared_apu = field_apu.prepare(builder);
                let prepared_joypad1 = field_joypad1.prepare(builder);
                let prepared_joypad2 = field_joypad2.prepare(builder);
                let prepared_cpu_cycles = field_cpu_cycles.prepare(builder, &0);
                let prepared_ppu_cycles = field_ppu_cycles.prepare(builder, &0);

                let mut table_writer: ::planus::table_writer::TableWriter<24> =
                    ::core::default::Default::default();
                if prepared_cpu_cycles.is_some() {
                    table_writer.write_entry::<u64>(8);
                }
                if prepared_ppu_cycles.is_some() {
                    table_writer.write_entry::<u64>(9);
                }
                if prepared_cpu.is_some() {
                    table_writer.write_entry::<::planus::Offset<self::CpuState>>(1);
                }
                if prepared_ram.is_some() {
                    table_writer.write_entry::<::planus::Offset<[u8]>>(2);
                }
                if prepared_mapper.is_some() {
                    table_writer.write_entry::<::planus::Offset<[u8]>>(3);
                }
                if prepared_ppu.is_some() {
                    table_writer.write_entry::<::planus::Offset<[u8]>>(4);
                }
                if prepared_apu.is_some() {
                    table_writer.write_entry::<::planus::Offset<[u8]>>(5);
                }
                if prepared_joypad1.is_some() {
                    table_writer.write_entry::<::planus::Offset<[u8]>>(6);
                }
                if prepared_joypad2.is_some() {
                    table_writer.write_entry::<::planus::Offset<[u8]>>(7);
                }
                if prepared_version.is_some() {
                    table_writer.write_entry::<u8>(0);
                }

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        if let ::core::option::Option::Some(prepared_cpu_cycles) =
                            prepared_cpu_cycles
                        {
                            object_writer.write::<_, _, 8>(&prepared_cpu_cycles);
                        }
                        if let ::core::option::Option::Some(prepared_ppu_cycles) =
                            prepared_ppu_cycles
                        {
                            object_writer.write::<_, _, 8>(&prepared_ppu_cycles);
                        }
                        if let ::core::option::Option::Some(prepared_cpu) = prepared_cpu {
                            object_writer.write::<_, _, 4>(&prepared_cpu);
                        }
                        if let ::core::option::Option::Some(prepared_ram) = prepared_ram {
                            object_writer.write::<_, _, 4>(&prepared_ram);
                        }
                        if let ::core::option::Option::Some(prepared_mapper) = prepared_mapper {
                            object_writer.write::<_, _, 4>(&prepared_mapper);
                        }
                        if let ::core::option::Option::Some(prepared_ppu) = prepared_ppu {
                            object_writer.write::<_, _, 4>(&prepared_ppu);
                        }
                        if let ::core::option::Option::Some(prepared_apu) = prepared_apu {
                            object_writer.write::<_, _, 4>(&prepared_apu);
                        }
                        if let ::core::option::Option::Some(prepared_joypad1) = prepared_joypad1 {
                            object_writer.write::<_, _, 4>(&prepared_joypad1);
                        }
                        if let ::core::option::Option::Some(prepared_joypad2) = prepared_joypad2 {
                            object_writer.write::<_, _, 4>(&prepared_joypad2);
                        }
                        if let ::core::option::Option::Some(prepared_version) = prepared_version {
                            object_writer.write::<_, _, 1>(&prepared_version);
                        }
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<NesState>> for NesState {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<NesState> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<NesState>> for NesState {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<NesState>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<NesState> for NesState {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<NesState> {
                NesState::create(
                    builder,
                    self.version,
                    &self.cpu,
                    &self.ram,
                    &self.mapper,
                    &self.ppu,
                    &self.apu,
                    &self.joypad1,
                    &self.joypad2,
                    self.cpu_cycles,
                    self.ppu_cycles,
                )
            }
        }

        /// Builder for serializing an instance of the [NesState] type.
        ///
        /// Can be created using the [NesState::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct NesStateBuilder<State>(State);

        impl NesStateBuilder<()> {
            /// Setter for the [`version` field](NesState#structfield.version).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn version<T0>(self, value: T0) -> NesStateBuilder<(T0,)>
            where
                T0: ::planus::WriteAsDefault<u8, u8>,
            {
                NesStateBuilder((value,))
            }

            /// Sets the [`version` field](NesState#structfield.version) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn version_as_default(self) -> NesStateBuilder<(::planus::DefaultValue,)> {
                self.version(::planus::DefaultValue)
            }
        }

        impl<T0> NesStateBuilder<(T0,)> {
            /// Setter for the [`cpu` field](NesState#structfield.cpu).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn cpu<T1>(self, value: T1) -> NesStateBuilder<(T0, T1)>
            where
                T1: ::planus::WriteAsOptional<::planus::Offset<self::CpuState>>,
            {
                let (v0,) = self.0;
                NesStateBuilder((v0, value))
            }

            /// Sets the [`cpu` field](NesState#structfield.cpu) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn cpu_as_null(self) -> NesStateBuilder<(T0, ())> {
                self.cpu(())
            }
        }

        impl<T0, T1> NesStateBuilder<(T0, T1)> {
            /// Setter for the [`ram` field](NesState#structfield.ram).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn ram<T2>(self, value: T2) -> NesStateBuilder<(T0, T1, T2)>
            where
                T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
            {
                let (v0, v1) = self.0;
                NesStateBuilder((v0, v1, value))
            }

            /// Sets the [`ram` field](NesState#structfield.ram) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn ram_as_null(self) -> NesStateBuilder<(T0, T1, ())> {
                self.ram(())
            }
        }

        impl<T0, T1, T2> NesStateBuilder<(T0, T1, T2)> {
            /// Setter for the [`mapper` field](NesState#structfield.mapper).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn mapper<T3>(self, value: T3) -> NesStateBuilder<(T0, T1, T2, T3)>
            where
                T3: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
            {
                let (v0, v1, v2) = self.0;
                NesStateBuilder((v0, v1, v2, value))
            }

            /// Sets the [`mapper` field](NesState#structfield.mapper) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn mapper_as_null(self) -> NesStateBuilder<(T0, T1, T2, ())> {
                self.mapper(())
            }
        }

        impl<T0, T1, T2, T3> NesStateBuilder<(T0, T1, T2, T3)> {
            /// Setter for the [`ppu` field](NesState#structfield.ppu).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn ppu<T4>(self, value: T4) -> NesStateBuilder<(T0, T1, T2, T3, T4)>
            where
                T4: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
            {
                let (v0, v1, v2, v3) = self.0;
                NesStateBuilder((v0, v1, v2, v3, value))
            }

            /// Sets the [`ppu` field](NesState#structfield.ppu) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn ppu_as_null(self) -> NesStateBuilder<(T0, T1, T2, T3, ())> {
                self.ppu(())
            }
        }

        impl<T0, T1, T2, T3, T4> NesStateBuilder<(T0, T1, T2, T3, T4)> {
            /// Setter for the [`apu` field](NesState#structfield.apu).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn apu<T5>(self, value: T5) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5)>
            where
                T5: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
            {
                let (v0, v1, v2, v3, v4) = self.0;
                NesStateBuilder((v0, v1, v2, v3, v4, value))
            }

            /// Sets the [`apu` field](NesState#structfield.apu) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn apu_as_null(self) -> NesStateBuilder<(T0, T1, T2, T3, T4, ())> {
                self.apu(())
            }
        }

        impl<T0, T1, T2, T3, T4, T5> NesStateBuilder<(T0, T1, T2, T3, T4, T5)> {
            /// Setter for the [`joypad1` field](NesState#structfield.joypad1).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn joypad1<T6>(self, value: T6) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6)>
            where
                T6: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
            {
                let (v0, v1, v2, v3, v4, v5) = self.0;
                NesStateBuilder((v0, v1, v2, v3, v4, v5, value))
            }

            /// Sets the [`joypad1` field](NesState#structfield.joypad1) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn joypad1_as_null(self) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, ())> {
                self.joypad1(())
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
            /// Setter for the [`joypad2` field](NesState#structfield.joypad2).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn joypad2<T7>(self, value: T7) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)>
            where
                T7: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
            {
                let (v0, v1, v2, v3, v4, v5, v6) = self.0;
                NesStateBuilder((v0, v1, v2, v3, v4, v5, v6, value))
            }

            /// Sets the [`joypad2` field](NesState#structfield.joypad2) to null.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn joypad2_as_null(self) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, ())> {
                self.joypad2(())
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)> {
            /// Setter for the [`cpu_cycles` field](NesState#structfield.cpu_cycles).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn cpu_cycles<T8>(
                self,
                value: T8,
            ) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
            where
                T8: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7) = self.0;
                NesStateBuilder((v0, v1, v2, v3, v4, v5, v6, v7, value))
            }

            /// Sets the [`cpu_cycles` field](NesState#structfield.cpu_cycles) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn cpu_cycles_as_default(
                self,
            ) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, ::planus::DefaultValue)>
            {
                self.cpu_cycles(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)> {
            /// Setter for the [`ppu_cycles` field](NesState#structfield.ppu_cycles).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn ppu_cycles<T9>(
                self,
                value: T9,
            ) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
            where
                T9: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8) = self.0;
                NesStateBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, value))
            }

            /// Sets the [`ppu_cycles` field](NesState#structfield.ppu_cycles) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn ppu_cycles_as_default(
                self,
            ) -> NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, ::planus::DefaultValue)>
            {
                self.ppu_cycles(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9>
            NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
        {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [NesState].
            #[inline]
            pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<NesState>
            where
                Self: ::planus::WriteAsOffset<NesState>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<self::CpuState>>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T3: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T4: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T5: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T6: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T7: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T8: ::planus::WriteAsDefault<u64, u64>,
                T9: ::planus::WriteAsDefault<u64, u64>,
            > ::planus::WriteAs<::planus::Offset<NesState>>
            for NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
        {
            type Prepared = ::planus::Offset<NesState>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<NesState> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<self::CpuState>>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T3: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T4: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T5: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T6: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T7: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T8: ::planus::WriteAsDefault<u64, u64>,
                T9: ::planus::WriteAsDefault<u64, u64>,
            > ::planus::WriteAsOptional<::planus::Offset<NesState>>
            for NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
        {
            type Prepared = ::planus::Offset<NesState>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<NesState>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<
                T0: ::planus::WriteAsDefault<u8, u8>,
                T1: ::planus::WriteAsOptional<::planus::Offset<self::CpuState>>,
                T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T3: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T4: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T5: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T6: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T7: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                T8: ::planus::WriteAsDefault<u64, u64>,
                T9: ::planus::WriteAsDefault<u64, u64>,
            > ::planus::WriteAsOffset<NesState>
            for NesStateBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
        {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<NesState> {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9) = &self.0;
                NesState::create(builder, v0, v1, v2, v3, v4, v5, v6, v7, v8, v9)
            }
        }

        /// Reference to a deserialized [NesState].
        #[derive(Copy, Clone)]
        pub struct NesStateRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

        impl<'a> NesStateRef<'a> {
            /// Getter for the [`version` field](NesState#structfield.version).
            #[inline]
            pub fn version(&self) -> ::planus::Result<u8> {
                ::core::result::Result::Ok(self.0.access(0, "NesState", "version")?.unwrap_or(2))
            }

            /// Getter for the [`cpu` field](NesState#structfield.cpu).
            #[inline]
            pub fn cpu(&self) -> ::planus::Result<::core::option::Option<self::CpuStateRef<'a>>> {
                self.0.access(1, "NesState", "cpu")
            }

            /// Getter for the [`ram` field](NesState#structfield.ram).
            #[inline]
            pub fn ram(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                self.0.access(2, "NesState", "ram")
            }

            /// Getter for the [`mapper` field](NesState#structfield.mapper).
            #[inline]
            pub fn mapper(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                self.0.access(3, "NesState", "mapper")
            }

            /// Getter for the [`ppu` field](NesState#structfield.ppu).
            #[inline]
            pub fn ppu(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                self.0.access(4, "NesState", "ppu")
            }

            /// Getter for the [`apu` field](NesState#structfield.apu).
            #[inline]
            pub fn apu(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                self.0.access(5, "NesState", "apu")
            }

            /// Getter for the [`joypad1` field](NesState#structfield.joypad1).
            #[inline]
            pub fn joypad1(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                self.0.access(6, "NesState", "joypad1")
            }

            /// Getter for the [`joypad2` field](NesState#structfield.joypad2).
            #[inline]
            pub fn joypad2(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                self.0.access(7, "NesState", "joypad2")
            }

            /// Getter for the [`cpu_cycles` field](NesState#structfield.cpu_cycles).
            #[inline]
            pub fn cpu_cycles(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(self.0.access(8, "NesState", "cpu_cycles")?.unwrap_or(0))
            }

            /// Getter for the [`ppu_cycles` field](NesState#structfield.ppu_cycles).
            #[inline]
            pub fn ppu_cycles(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(self.0.access(9, "NesState", "ppu_cycles")?.unwrap_or(0))
            }
        }

        impl<'a> ::core::fmt::Debug for NesStateRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("NesStateRef");
                f.field("version", &self.version());
                if let ::core::option::Option::Some(field_cpu) = self.cpu().transpose() {
                    f.field("cpu", &field_cpu);
                }
                if let ::core::option::Option::Some(field_ram) = self.ram().transpose() {
                    f.field("ram", &field_ram);
                }
                if let ::core::option::Option::Some(field_mapper) = self.mapper().transpose() {
                    f.field("mapper", &field_mapper);
                }
                if let ::core::option::Option::Some(field_ppu) = self.ppu().transpose() {
                    f.field("ppu", &field_ppu);
                }
                if let ::core::option::Option::Some(field_apu) = self.apu().transpose() {
                    f.field("apu", &field_apu);
                }
                if let ::core::option::Option::Some(field_joypad1) = self.joypad1().transpose() {
                    f.field("joypad1", &field_joypad1);
                }
                if let ::core::option::Option::Some(field_joypad2) = self.joypad2().transpose() {
                    f.field("joypad2", &field_joypad2);
                }
                f.field("cpu_cycles", &self.cpu_cycles());
                f.field("ppu_cycles", &self.ppu_cycles());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<NesStateRef<'a>> for NesState {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: NesStateRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    version: ::core::convert::TryInto::try_into(value.version()?)?,
                    cpu: if let ::core::option::Option::Some(cpu) = value.cpu()? {
                        ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                            ::core::convert::TryInto::try_into(cpu)?,
                        ))
                    } else {
                        ::core::option::Option::None
                    },
                    ram: value.ram()?.map(|v| v.to_vec()),
                    mapper: value.mapper()?.map(|v| v.to_vec()),
                    ppu: value.ppu()?.map(|v| v.to_vec()),
                    apu: value.apu()?.map(|v| v.to_vec()),
                    joypad1: value.joypad1()?.map(|v| v.to_vec()),
                    joypad2: value.joypad2()?.map(|v| v.to_vec()),
                    cpu_cycles: ::core::convert::TryInto::try_into(value.cpu_cycles()?)?,
                    ppu_cycles: ::core::convert::TryInto::try_into(value.ppu_cycles()?)?,
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for NesStateRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for NesStateRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location("[NesStateRef]", "get", buffer.offset_from_start)
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<NesState>> for NesState {
            type Value = ::planus::Offset<NesState>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<NesState>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for NesStateRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[NesStateRef]", "read_as_root", 0)
                })
            }
        }
    }
}
