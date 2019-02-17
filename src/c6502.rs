#![allow(unused_imports)]
#![allow(non_upper_case_globals)]

use crate::common::{run_clocks, Clocked, get_bit};
use crate::mapper::AddressSpace;
use crate::mapper::NullAddressSpace;

// const ADDRESS_RESET:u16 = 0xFFFC;
const ADDRESS_RESET:u16 = 0xC000;
const ADDRESS_TEST_PROGRAM:u16 = 0xC000;

// Ricoh 2A03, a variation of the 6502
pub struct C6502 {
    acc: u8,
    x: u8,
    y: u8,
    pc: u16,
    sp: u8,
    carry: bool,
    zero: bool,
    interruptd: bool,
    decimal: bool,
    overflow: bool,
    negative: bool,
    pub mapper: Box<AddressSpace>,
    pub counter: usize,
    pub clocks: usize,
    is_tracing: bool,
}

impl C6502 {
    pub fn new() -> C6502 {
        let mapper:NullAddressSpace = NullAddressSpace::new();
        return C6502 {
            acc: 0,
            x: 0,
            y: 0,
            pc: ADDRESS_RESET,
            sp: 0xfd,
            carry: false,
            zero: false,
            interruptd: false,
            decimal: false,
            overflow: false,
            negative: false,
            mapper: Box::new(mapper),
            counter: 0,
            clocks: 0,
            is_tracing: true,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum Operation {
    ADC, AND, ASL, BCC,
    BCS, BEQ, BIT, BMI,
    BNE, BPL, BRK, BVC,
    BVS, CLC, CLD, CLI,
    CLV, CMP, CPX, CPY,
    DEC, DEX, DEY, EOR,
    INC, INX, INY, JMP,
    JSR, LDA, LDX, LDY,
    LSR, NOP, ORA, PHA,
    PHP, PLA, PLP, ROL,
    ROR, RTI, RTS, SBC,
    SEC, SED, SEI, STA,
    STX, STY, TAX, TAY,
    TSX, TXA, TXS, TYA,
    // "Extra" opcodes
    KIL,ISC,DCP,AXS,
    LAS,LAX,AHX,SAX,
    XAA,SHX,RRA,TAS,
    SHY,ARR,SRE,ALR,
    RLA,ANC,SLO,
}

#[derive(Copy,Clone,Debug)]
enum AddressingMode {
    Immediate,
    ZeroPage,ZeroPageX,ZeroPageY,
    Absolute,AbsoluteX,AbsoluteY,
    Indirect,IndirectX,IndirectY,
    Relative,
    Accumulator,
    Implicit,
}

use Operation::*;
use AddressingMode::*;

const STACK_PAGE:u16 = 0x0100;

type CycleCount = u8;

//
const abs:AddressingMode = Absolute;
const acc:AddressingMode = Accumulator;
const imm:AddressingMode = Immediate;
const imp:AddressingMode = Implicit;
const izx:AddressingMode = IndirectX;
const izy:AddressingMode = IndirectY;
const zp:AddressingMode  = ZeroPage;
const zpx:AddressingMode = ZeroPageX;
const zpy:AddressingMode = ZeroPageY;
const rel:AddressingMode = Relative;
const abx:AddressingMode = AbsoluteX;
const aby:AddressingMode = AbsoluteY;
const ind:AddressingMode = Indirect;

// Opcode table: http://www.oxyron.de/html/opcodes02.html
const OPCODE_TABLE: [(Operation, AddressingMode, CycleCount, CycleCount);256] =
    // TODO Audit each record to see that it was input correctly
    // (Operation, addressing mode, clock cycles, extra clock cycles if page boundary crossed)
    [   // 0x
        (BRK, imp, 7, 0), // x0
        (ORA, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (SLO, izx, 8, 0), // x3
        (NOP, zp,  3, 0), // x4
        (ORA, zp,  3, 0), // x5
        (ASL, zp,  5, 0), // x6
        (SLO, zp,  5, 0), // x7
        (PHP, imp, 3, 0), // x8
        (ORA, imm, 2, 0), // x9
        (ASL, acc, 2, 0), // xA
        (ANC, imm, 2, 0), // xB
        (NOP, abs, 4, 0), // xC
        (ORA, abs, 4, 0), // xD
        (ASL, abs, 6, 0), // xE
        (SLO, abs, 6, 0), // xF
        // 1x
        (BPL, rel, 2, 1), // x0
        (ORA, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (SLO, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (ORA, zpx, 4, 0), // x5
        (ASL, zpx, 6, 0), // x6
        (SLO, zpx, 6, 0), // x7
        (CLC, imp, 2, 0), // x8
        (ORA, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (SLO, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (ORA, abx, 4, 1), // xD
        (ASL, abx, 7, 0), // xE
        (SLO, abx, 7, 0), // xF
        // 2x
        (JSR, abs, 6, 0), // x0
        (AND, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (RLA, izx, 8, 0), // x3
        (BIT, zp,  3, 0), // x4
        (AND, zp,  3, 0), // x5
        (ROL, zp,  5, 0), // x6
        (RLA, zp,  5, 0), // x7
        (PLP, imp, 4, 0), // x8
        (AND, imm, 2, 0), // x9
        (ROL, acc, 2, 0), // xA
        (ANC, imm, 2, 0), // xB
        (BIT, abs, 4, 0), // xC
        (AND, abs, 4, 0), // xD
        (ROL, abs, 6, 0), // xE
        (RLA, abs, 6, 0), // xF
        // 3x
        (BMI, rel, 2, 1), // x0
        (AND, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (RLA, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (AND, zpx, 4, 0), // x5
        (ROL, zpx, 6, 0), // x6
        (RLA, zpx, 6, 0), // x7
        (SEC, imp, 2, 0), // x8
        (AND, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (RLA, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (AND, abx, 4, 1), // xD
        (ROL, abx, 7, 0), // xE
        (RLA, abx, 7, 0), // xF
        // 4x
        (RTI, imp, 6, 0), // x0
        (EOR, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (SRE, izx, 8, 0), // x3
        (NOP, zp,  3, 0), // x4
        (EOR, zp,  3, 0), // x5
        (LSR, zp,  5, 0), // x6
        (SRE, zp,  5, 0), // x7
        (PHA, imp, 3, 0), // x8
        (EOR, imm, 2, 0), // x9
        (LSR, imp, 2, 0), // xA
        (ALR, imm, 2, 0), // xB
        (JMP, abs, 3, 0), // xC
        (EOR, abs, 4, 0), // xD
        (LSR, abs, 6, 0), // xE
        (SRE, abs, 6, 0), // xF
        // 5x
        (BVC, rel, 2, 1), // x0
        (EOR, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (SRE, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (EOR, zpx, 4, 0), // x5
        (LSR, zpx, 6, 0), // x6
        (SRE, zpx, 6, 0), // x7
        (CLI, imp, 2, 0), // x8
        (EOR, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (SRE, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (EOR, abx, 4, 1), // xD
        (LSR, abx, 7, 0), // xE
        (SRE, abx, 7, 0), // xF
        // 6x
        (RTS, imp, 6, 0), // x0
        (ADC, izx, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (RRA, izx, 8, 0), // x3
        (NOP, zp,  3, 0), // x4
        (ADC, zp,  3, 0), // x5
        (ROR, zp,  5, 0), // x6
        (RRA, zp,  5, 0), // x7
        (PLA, imp, 4, 0), // x8
        (ADC, imm, 2, 0), // x9
        (ROR, imp, 2, 0), // xA
        (ARR, imm, 2, 0), // xB
        (JMP, ind, 5, 0), // xC
        (ADC, abs, 4, 0), // xD
        (ROR, abs, 6, 0), // xE
        (RRA, abs, 6, 0), // xF
        // 7x
        (BVS, rel, 2, 1), // x0
        (ADC, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (RRA, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (ADC, zpx, 4, 0), // x5
        (ROR, zpx, 6, 0), // x6
        (RRA, zpx, 6, 0), // x7
        (SEI, imp, 2, 0), // x8
        (ADC, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (RRA, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (ADC, abx, 4, 1), // xD
        (ROR, abx, 7, 0), // xE
        (RRA, abx, 7, 0), // xF
        // 8x
        (NOP, imm, 2, 0), // x0
        (STA, izx, 6, 0), // x1
        (NOP, imm, 2, 0), // x2
        (SAX, izx, 6, 0), // x3
        (STY, zp,  3, 0), // x4
        (STA, zp,  3, 0), // x5
        (STX, zp,  3, 0), // x6
        (SAX, zp,  3, 0), // x7
        (DEY, imp, 2, 0), // x8
        (NOP, imm, 2, 0), // x9
        (TXA, imp, 2, 0), // xA
        (XAA, imm, 2, 1), // xB
        (STY, abs, 4, 0), // xC
        (STA, abs, 4, 0), // xD
        (STX, abs, 4, 0), // xE
        (SAX, abs, 4, 0), // xF
        // 9x
        (BCC, rel, 2, 1), // x0
        (STA, izy, 6, 0), // x1
        (KIL, imp, 0, 0), // x2
        (AHX, izy, 6, 0), // x3
        (STY, zpx, 4, 0), // x4
        (STA, zpx, 4, 0), // x5
        (STX, zpy, 4, 0), // x6
        (SAX, zpy, 4, 0), // x7
        (TYA, imp, 2, 0), // x8
        (STA, aby, 5, 0), // x9
        (TXS, imp, 2, 0), // xA
        (TAS, aby, 5, 0), // xB
        (SHY, abx, 5, 0), // xC
        (STA, abx, 5, 0), // xD
        (SHX, aby, 5, 0), // xE
        (AHX, aby, 5, 0), // xF
        // Ax
        (LDY, imm, 2, 0), // x0
        (LDA, izx, 6, 0), // x1
        (LDX, imm, 2, 0), // x2
        (LAX, izx, 6, 0), // x3
        (LDY, zp,  3, 0), // x4
        (LDA, zp,  3, 0), // x5
        (LDX, zp,  3, 0), // x6
        (LAX, zp,  3, 0), // x7
        (TAY, imp, 2, 0), // x8
        (LDA, imm, 2, 0), // x9
        (TAX, imp, 2, 0), // xA
        (LAX, imm, 2, 0), // xB
        (LDY, abs, 4, 0), // xC
        (LDA, abs, 4, 0), // xD
        (LDX, abs, 4, 0), // xE
        (LAX, abs, 4, 0), // xF
        // Bx
        (BCS, rel, 2, 1), // x0
        (LDA, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (LAX, izy, 5, 1), // x3
        (LDY, zpx, 4, 0), // x4
        (LDA, zpx, 4, 0), // x5
        (LDX, zpy, 4, 0), // x6
        (LAX, zpy, 4, 0), // x7
        (CLV, imp, 2, 0), // x8
        (LDA, aby, 4, 1), // x9
        (TSX, imp, 2, 0), // xA
        (LAS, aby, 4, 1), // xB
        (LDY, abx, 4, 1), // xC
        (LDA, abx, 4, 1), // xD
        (LDX, aby, 4, 1), // xE
        (LAX, aby, 4, 1), // xF
        // Cx
        (CPY, imm, 2, 0), // x0
        (CMP, izx, 6, 0), // x1
        (NOP, imm, 2, 0), // x2
        (DCP, izx, 8, 0), // x3
        (CPY, zp,  3, 0), // x4
        (CMP, zp,  3, 0), // x5
        (DEC, zp,  5, 0), // x6
        (DCP, zp,  5, 0), // x7
        (INY, imp, 2, 0), // x8
        (CMP, imm, 2, 0), // x9
        (DEX, imp, 2, 0), // xA
        (AXS, imm, 2, 0), // xB
        (CPY, abs, 4, 0), // xC
        (CMP, abs, 4, 0), // xD
        (DEC, abs, 6, 0), // xE
        (DCP, abs, 6, 0), // xF
        // Dx
        (BNE, rel, 2, 1), // x0
        (CMP, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (DCP, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (CMP, zpx, 4, 0), // x5
        (DEC, zpx, 6, 0), // x6
        (DCP, zpx, 6, 0), // x7
        (CLD, imp, 2, 0), // x8
        (CMP, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (DCP, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (CMP, abx, 4, 1), // xD
        (DEC, abx, 7, 0), // xE
        (DCP, abx, 7, 0), // xF
        // Ex
        (CPX, imm, 2, 0), // x0
        (SBC, izx, 6, 0), // x1
        (NOP, imm, 2, 0), // x2
        (ISC, izx, 8, 0), // x3
        (CPX, zp,  3, 0), // x4
        (SBC, zp,  3, 0), // x5
        (INC, zp,  5, 0), // x6
        (ISC, zp,  5, 0), // x7
        (INX, imp, 2, 0), // x8
        (SBC, imm, 2, 0), // x9
        (NOP, imp, 2, 0), // xA
        (SBC, imm, 2, 0), // xB
        (CPX, abs, 4, 0), // xC
        (SBC, abs, 4, 0), // xD
        (INC, abs, 6, 0), // xE
        (ISC, abs, 6, 0), // xF
        // Fx
        (BEQ, rel, 2, 1), // x0
        (SBC, izy, 5, 1), // x1
        (KIL, imp, 0, 0), // x2
        (ISC, izy, 8, 0), // x3
        (NOP, zpx, 4, 0), // x4
        (SBC, zpx, 4, 0), // x5
        (INC, zpx, 6, 0), // x6
        (ISC, zpx, 6, 0), // x7
        (SED, imp, 2, 0), // x8
        (SBC, aby, 4, 1), // x9
        (NOP, imp, 2, 0), // xA
        (ISC, aby, 7, 0), // xB
        (NOP, abx, 4, 1), // xC
        (SBC, abx, 4, 1), // xD
        (INC, abx, 7, 0), // xE
        (ISC, abx, 7, 0), // xF
        ];

type WriteTarget = Option<u16>;

#[derive(Copy, Clone, Debug)]
struct Instruction {
    op: Operation,
    mode: AddressingMode,
    mode_args: u16,
    write_target: WriteTarget,
}

impl Instruction {
    fn should_advance_pc(&self) -> bool {
        match self.op {
            JMP => false,
            JSR => false,
            RTS => false,
            _ => true,
        }
    }
}

impl Clocked for C6502 {
    fn clock(&mut self) {
        self.counter += 1;
        let ptr = self.pc;
        let (i, num_bytes) = self.decode_instruction();
        if self.is_tracing
        { self.print_trace_line(num_bytes, &i); }
        //eprintln!("DEBUG - Instruction - {:x} {:?}({:?} bytes)", ptr, i, num_bytes);
        self.pc = self.pc.wrapping_add(num_bytes);
        self.execute_instruction(i);
    }
}

impl C6502 {
    fn print_trace_line(&self, num_bytes:u16, i:&Instruction) {
        let ptr = self.pc;
        let bytes:u32 = match num_bytes {
            1 => self.peek(ptr) as u32,
            2 => ((self.peek(ptr) as u32)                 << 8) +
                 ((self.peek(ptr.wrapping_add(1)) as u32) << 0),
            3 => ((self.peek(ptr) as u32)                 << 16) +
                 ((self.peek(ptr.wrapping_add(1)) as u32) << 8) +
                 ((self.peek(ptr.wrapping_add(2)) as u32) << 0),
            _ => panic!("print_trace_line - Unexpected num_bytes {:?} {:?}", num_bytes, i),
        };
        eprintln!("{:4}: {:x} {:<8x} {:?} A:{:2x} X:{:2x} Y:{:2x} P:{:2x} SP:{:2x} I:{:?}",
                  self.counter, self.pc, bytes, i.op, self.acc, self.x, self.y, self.status_register_byte(true), self.sp, i);
    }

    fn decode_instruction(&self) -> (Instruction, u16) {
        let ptr = self.pc;
        let opcode = self.peek(ptr) as usize;
        //eprintln!("DEBUG - Opcode - {:x}", opcode);
        let (op, mode, clocks, page_clocks) = OPCODE_TABLE[opcode];
        let (mode_args, write_target, num_arg_bytes) = self.decode_addressing_mode(mode, ptr.wrapping_add(1));
        let instruction = Instruction { op, mode, mode_args, write_target };
        return (instruction, 1 + num_arg_bytes);
    }

    fn read_write_target(&self, write_target: Option<u16>) -> u8 {
        match write_target {
            None => self.acc,
            Some(ptr) => self.peek(ptr),
        }
    }

    fn store_write_target(&mut self, v: u8, write_target: Option<u16>) {
        match write_target {
            None => { self.acc = v },
            Some(ptr) => { self.poke(ptr, v); },
        }
    }

    fn execute_instruction(&mut self, i: Instruction) {
        let v = i.mode_args as u8;

        match i.op {
            ADC => { self.execute_adc(v) },
            AND => { self.execute_and(v) },
            ASL => { self.execute_asl(v) },
            BCC => { self.execute_bcc(v) },
            BCS => { self.execute_bcs(v) },
            BEQ => { self.execute_beq(v) },
            BIT => { self.execute_bit(v) },
            BMI => { self.execute_bmi(v) },
            BNE => { self.execute_bne(v) },
            BPL => { self.execute_bpl(v) },
            BRK => { self.execute_brk() },
            BVC => { self.execute_bvc(v) },
            BVS => { self.execute_bvs(v) },
            CLC => { self.execute_clc() },
            CLD => { self.execute_cld() },
            CLI => { self.execute_cli() },
            CLV => { self.execute_clv() },
            CMP => { self.execute_cmp(v) },
            CPX => { self.execute_cpx(v) },
            CPY => { self.execute_cpy(v) },
            DEC => { let r = self.read_write_target(i.write_target);
                     let w = self.execute_dec(r);
                     self.store_write_target(w, i.write_target);
            },
            DEX => { self.execute_dex() },
            DEY => { self.execute_dey() },
            EOR => { self.execute_eor(v) },
            INC => { let r = self.read_write_target(i.write_target);
                     let w = self.execute_inc(r);
                     self.store_write_target(w, i.write_target);
            },
            INX => { self.execute_inx() },
            INY => { self.execute_iny() },
            JMP => { self.execute_jmp(i.write_target.unwrap()) },
            JSR => { self.execute_jsr(i.write_target.unwrap()) },
            LDA => { self.execute_lda(v) },
            LDX => { self.execute_ldx(v) },
            LDY => { self.execute_ldy(v) },
            LSR => { let r = self.read_write_target(i.write_target);
                     let w = self.execute_lsr(r);
                     self.store_write_target(w, i.write_target);
            },
            NOP => { self.execute_nop() },
            ORA => { self.execute_ora(v) },
            PHA => { self.execute_pha() },
            PHP => { self.execute_php() },
            PLA => { self.execute_pla() },
            PLP => { self.execute_plp() },
            ROL => { let r = self.read_write_target(i.write_target);
                     let w = self.execute_rol(r);
                     self.store_write_target(w, i.write_target);
            },
            ROR => { let r = self.read_write_target(i.write_target);
                     let w = self.execute_ror(r);
                     self.store_write_target(w, i.write_target); },
            RTI => { self.execute_rti() },
            RTS => { self.execute_rts() },
            SBC => { self.execute_sbc(v) },
            SEC => { self.execute_sec() },
            SED => { self.execute_sed() },
            SEI => { self.execute_sei() },
            STA => { let w = self.acc;
                     self.store_write_target(w, i.write_target) },
            STX => { let w = self.x;
                     self.store_write_target(w, i.write_target) },
            STY => { let w = self.y;
                     self.store_write_target(w, i.write_target) },
            TAX => { self.execute_tax() },
            TAY => { self.execute_tay() },
            TSX => { self.execute_tsx() },
            TXA => { self.execute_txa() },
            TXS => { self.execute_txs() },
            TYA => { self.execute_tya() },
            KIL => { panic!("KIL instruction encountered") },
            _ => { self.execute_unimplemented(i.op) },
        }
    }

    // Returns the instruction arguments and the number of bytes after the opcode they took to store.
    fn decode_addressing_mode(&self, mode: AddressingMode, ptr: u16) -> (u16, Option<u16>, u16) {
        match mode {
            // (Value, Address, Additional Bytes)
            Immediate   => (self.peek(ptr) as u16, Some(ptr), 1),
            ZeroPage    => {
                let addr = self.peek(ptr) as u16;
                (self.peek(addr) as u16, Some(addr), 1)
            },
            ZeroPageX   => {
                let addr = self.peek(ptr).wrapping_add(self.x) as u16;
                (self.peek(addr) as u16, Some(addr), 1)
            },
            ZeroPageY   => {
                let addr = self.peek(ptr).wrapping_add(self.y) as u16;
                (self.peek(addr) as u16, Some(addr), 1)
            },
            Absolute    => {
                let addr = self.peek16(ptr);
                (self.peek(addr) as u16, Some(addr), 2)
            },
            AbsoluteX   => {
                let addr = ptr + (self.x as u16);
                (self.peek(addr) as u16, Some(addr), 1)
            },
            AbsoluteY   => {
                let addr = ptr + (self.y as u16);
                (self.peek(addr) as u16, Some(addr), 1)
            },
            Indirect    => {
                let addr = self.peek16(ptr);
                (self.peek(addr) as u16, Some(addr), 2)
            },
            IndirectX   => {
                let zp_addr = self.peek(ptr).wrapping_add(self.x);
                let addr = self.peek_zero16(zp_addr);
                (self.peek(addr) as u16, Some(addr), 1)
            },
            IndirectY   => {
                let zp_addr = self.peek(ptr);
                let addr = self.peek_zero16(zp_addr).wrapping_add(self.y as u16);
                (self.peek(addr) as u16, Some(addr), 1)
            },
            Relative    => {
                (self.peek(ptr) as u16, Some(ptr), 1)
            }
            Accumulator => (self.acc as u16, None, 0),
            Implicit    => (0xDEAD, None, 0),
        }
    }
}

// BEGIN instructions

impl C6502 {
    fn execute_adc(&mut self, v: u8) {
        let (x1, o1) = v.overflowing_add(self.acc);
        let (x2, o2) = x1.overflowing_add(self.carry as u8);
        self.carry = o1 | o2;
        let signed_sum = (v as i8 as i16) + (self.acc as i8 as i16) + (self.carry as i16);
        self.acc = x2;
        self.overflow = (signed_sum < -128) || (signed_sum > 127);
        self.update_accumulator_flags();
    }

    fn execute_and(&mut self, v: u8) {
        self.acc &= v;
        self.update_accumulator_flags();
    }

    fn execute_asl(&mut self, v: u8) {
        let (x, o) = v.overflowing_mul(2);
        self.carry = get_bit(v, 7) > 0;
        self.acc = x;
        self.update_accumulator_flags();
    }

    fn execute_branch(&mut self, v: u8) {
        self.pc += (v as i8) as u16;
    }

    fn execute_bcc(&mut self, v: u8) {
        eprintln!("DEBUG - CARRY - {} {}", self.carry, 0xf9 & 0b00000001 > 0);
        if !self.carry
        { self.execute_branch(v); }
    }

    fn execute_bcs(&mut self, v: u8) {
        if self.carry
        { self.execute_branch(v); }
    }

    fn execute_beq(&mut self, v: u8) {
        if self.zero
        { self.execute_branch(v); }
    }

    fn execute_bit(&mut self, v: u8) {
        let x = v & self.acc;
        self.negative = 0b10000000 & v > 0;
        self.overflow = 0b01000000 & v > 0;
        self.zero = x == 0;
        eprintln!("V:{} A:{} N:{} O:{} Z:{}",
                  v, self.acc, self.negative, self.overflow, self.zero);
    }

    fn execute_bmi(&mut self, v: u8) {
        if self.negative
        { self.execute_branch(v); }
    }

    fn execute_bne(&mut self, v: u8) {
        if !self.zero
        { self.execute_branch(v); }
    }

    fn execute_bpl(&mut self, v: u8) {
        if !self.negative
        { self.execute_branch(v); }
    }

    fn execute_brk(&mut self) {
        let pc = self.pc;
        self.push_stack16(pc);
        let sr = self.status_register_byte(true);
        self.push_stack(sr);
        self.pc = self.peek16(0xFFFE);
    }

    fn execute_bvc(&mut self, v: u8) {
        if !self.overflow
        { self.execute_branch(v); }
    }

    fn execute_bvs(&mut self, v: u8) {
        if self.overflow
        { self.execute_branch(v); }
    }

    fn execute_clc(&mut self) {
        self.carry = false;
    }

    fn execute_cld(&mut self) {
        self.decimal = false;
    }

    fn execute_cli(&mut self) {
        self.interruptd = false;
    }

    fn execute_clv(&mut self) {
        self.overflow = false;
    }

    fn execute_compare(&mut self, v1: u8, v2: u8) {
        let result = v1.wrapping_sub(v2);
        self.carry = v1 >= v2;
        self.zero = v1 == v2;
        self.negative = is_negative(result);
        eprintln!("DEBUG - COMPARE - {} {} {} {} {}", v1, v2, self.carry, self.zero, self.negative);
    }

    fn execute_cmp(&mut self, v: u8) {
        let a = self.acc;
        self.execute_compare(a, v);
    }

    fn execute_cpx(&mut self, v: u8) {
        let x = self.x;
        self.execute_compare(x, v);
    }

    fn execute_cpy(&mut self, v: u8) {
        let y = self.y;
        self.execute_compare(y, v);
    }

    fn execute_dec(&mut self, v: u8) -> u8 {
        let ret = v.wrapping_sub(1);
        self.update_result_flags(ret);
        return ret;
    }

    fn execute_dex(&mut self) {
        let x = self.x;
        self.x = self.execute_dec(x);
    }

    fn execute_dey(&mut self) {
        let y = self.y;
        self.y = self.execute_dec(y);
    }

    fn execute_eor(&mut self, v: u8) {
        self.acc ^= v;
        self.update_accumulator_flags();
    }

    fn execute_inc(&mut self, v: u8) -> u8 {
        let ret = v.wrapping_add(1);
        self.update_result_flags(ret);
        return ret;
    }

    fn execute_inx(&mut self) {
        let x = self.x;
        self.x = self.execute_inc(x);
    }

    fn execute_iny(&mut self) {
        let y = self.y;
        self.y = self.execute_inc(y);
    }

    fn execute_jmp(&mut self, ptr: u16) {
        self.pc = ptr;
    }

    fn execute_jsr(&mut self, ptr: u16) {
        let pc = self.pc;
        self.push_stack16(pc.wrapping_sub(1));
        self.pc = ptr;
    }

    fn execute_lda(&mut self, v: u8) {
        self.acc = v;
        self.update_accumulator_flags();
    }

    fn execute_ldx(&mut self, v: u8) {
        self.x = v;
        self.update_result_flags(v);
    }

    fn execute_ldy(&mut self, v: u8) {
        self.y = v;
        self.update_result_flags(v);
    }

    fn execute_lsr(&mut self, v: u8) -> u8 {
        self.carry = v & 0b00000001 > 0;
        let ret = v.wrapping_shr(1);
        self.update_result_flags(ret);
        return ret;
    }

    fn execute_nop(&mut self) { }

    fn execute_ora(&mut self, v: u8) {
        self.acc |= v;
        self.update_accumulator_flags();
    }

    fn execute_pha(&mut self) {
        let x = self.acc;
        self.push_stack(x);
    }

    fn execute_php(&mut self) {
        let x = self.status_register_byte(true);
        self.push_stack(x);
    }

    fn execute_pla(&mut self) {
        self.acc = self.pop_stack();
        self.update_accumulator_flags();
    }

    fn execute_plp(&mut self) {
        let x = self.pop_stack();
        self.set_status_register_from_byte(x);
    }

    fn execute_rol(&mut self, v: u8) -> u8 {
        self.carry = v & 0b10000000 > 0;
        let ret = v.rotate_left(1);
        self.update_result_flags(ret);
        return ret
    }

    fn execute_ror(&mut self, v: u8) -> u8 {
        let mut ret = v.rotate_right(1);
        if self.carry
        { ret |= 1 << 7 }
        else { ret &= !(1 << 7) }
        self.carry = v & 0b00000001 > 0;
        self.update_result_flags(ret);
        return ret;
    }

    fn execute_rti(&mut self) {
        let x = self.pop_stack();
        self.set_status_register_from_byte(x);
        self.pc = self.pop_stack16();
    }

    fn execute_rts(&mut self) {
        self.pc = self.pop_stack16().wrapping_add(1);
    }

    fn execute_sbc(&mut self, v: u8) {
        let (x1, o1) = self.acc.overflowing_sub(v);
        let (x2, o2) = x1.overflowing_sub(!self.carry as u8);
        self.carry = !(o1 | o2);
        let signed_sub = (self.acc as i8 as i16) - (v as i8 as i16) - (1 - (self.carry as i16));
        self.acc = x2;
        self.overflow = (signed_sub < -128) || (signed_sub > 127);
        self.update_accumulator_flags();
    }

    fn execute_sec(&mut self) {
        self.carry = true;
    }

    fn execute_sed(&mut self) {
        self.decimal = true;
    }

    fn execute_sei(&mut self) {
        self.interruptd = true;
    }

    // fn execute_sta(&mut self, v: u8) { //
    //     *v = self.acc;
    // }

    // fn execute_stx(&mut self, v: &u8) {
    //     *v = self.x;
    // }

    // fn execute_sty(&mut self, v: &u8) {
    //     *v = self.y;
    // }

    fn execute_tax(&mut self) {
        self.x = self.acc;
        let x = self.x;
        self.update_result_flags(x);
    }

    fn execute_tay(&mut self) {
        self.y = self.acc;
        let y = self.y;
        self.update_result_flags(y);
    }

    fn execute_tsx(&mut self) {
        self.x = self.sp;
        let x = self.x;
        self.update_result_flags(x);
    }

    fn execute_txa(&mut self) {
        self.acc = self.x;
        self.update_accumulator_flags();
    }

    fn execute_txs(&mut self) {
        self.sp = self.x;
    }

    fn execute_tya(&mut self) {
        self.acc = self.y;
        self.update_accumulator_flags();
    }
    fn execute_unimplemented(&mut self, op: Operation) {
        panic!("Unimplemented operation: {:?}", op);
    }
}
// END instructions

impl C6502 {
    fn push_stack(&mut self, v: u8) {
        let sp = self.sp;
        self.poke_offset(STACK_PAGE, sp as i16, v);
        self.sp = self.sp.wrapping_sub(1);
    }

    // fn peek_stack(&self) -> u8{
    //     let v = self.pop_stack();
    //     self.push_stack(v);
    //     return v;
    //     //self.peek_offset(STACK_PAGE, self.sp.wrapping_add(1) as i16);
    // }

    fn pop_stack(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        return self.peek_offset(STACK_PAGE, self.sp as i16);
    }

    fn push_stack16(&mut self, v: u16) {
        self.push_stack(((v & 0xFF00) >> 8) as u8);
        self.push_stack((v & 0xFF) as u8);
    }

    fn pop_stack16(&mut self) -> u16 {
        let lsb = self.pop_stack() as u16;
        let msb = self.pop_stack() as u16;
        return (msb << 8) + lsb;
    }
    fn peek_zero16(&self, ptr:u8) -> u16 {
        let lsb = self.peek(ptr as u16) as u16;
        let msb = self.peek(ptr.wrapping_add(1) as u16) as u16;
        return (msb << 8) + lsb;
    }
}

impl AddressSpace for C6502 {
    fn peek(&self, ptr:u16) -> u8{
        let v = self.mapper.peek(ptr);
        eprintln!("PEEK {} {}", ptr, v);
        return v;
    }
    fn poke(&mut self, ptr:u16, v:u8) {
        eprintln!("POKE {} {}", ptr, v);
        return self.mapper.poke(ptr, v);
    }
}

impl C6502 {
    fn update_result_flags(&mut self, v: u8) {
        self.zero = v == 0;
        self.negative = is_negative(v);
    }

    fn update_accumulator_flags(&mut self) {
        let x = self.acc;
        self.update_result_flags(x);
    }

    fn status_register_byte(&self, is_instruction: bool) -> u8 {
        let result =
            ((self.carry      as u8) << 0) +
            ((self.zero       as u8) << 1) +
            ((self.interruptd as u8) << 2) +
            ((self.decimal    as u8) << 3) +
            (0                       << 4) + // Break flag
            ((if is_instruction {1} else {0}) << 5) +
            ((self.overflow   as u8) << 6) +
            ((self.negative   as u8) << 7);
        return result;
    }

    fn set_status_register_from_byte(&mut self, v: u8) {
        self.carry      = v & 0b00000001 > 0;
        self.zero       = v & 0b00000010 > 0;
        self.interruptd = v & 0b00000100 > 0;
        self.decimal    = v & 0b00001000 > 0;
        // Break isn't a real register
        // Bit 5 is unused
        self.overflow   = v & 0b01000000 > 0;
        self.negative   = v & 0b10000000 > 0;
    }
}

fn is_negative(v: u8) -> bool {
    return v >= 128;
}

mod tests {
    use super::*;

    use crate::mapper::Ram;

    fn create_test_cpu(program:&Vec<u8>) -> C6502 {
        let mut ret = C6502::new();
        ret.is_tracing = false;
        let mapper = Ram::new(65536);
        ret.mapper = Box::new(mapper);
        for (byte, idx) in program.iter().zip(0..65536) {
            ret.poke(ADDRESS_TEST_PROGRAM + idx as u16, *byte)
        }
        ret.pc = ADDRESS_TEST_PROGRAM;
        return ret;
    }

    #[test]
    fn test_bit() {
        let program:Vec<u8> = vec!(0xa9, 0xff, // LDA #255
                                   0x85, 0x01, // STA $01
                                   0x24, 0x01, // BIT $01
        );
        let mut c = create_test_cpu(&program);
        c.clock();
        c.clock();
        c.clock();
        eprintln!("STATUS REGISTER {}", c.status_register_byte(true));
        assert!(c.overflow);
    }
    #[test]
    fn test_subroutine() {
        let program:Vec<u8> =
            vec!(0x20, 0x03, 0xC0, // JSR $C003
                 0x60,             // RTS
            );
        let mut c = create_test_cpu(&program);
        c.is_tracing = true;
        c.clock();
        c.clock();
        assert_eq!(c.pc, 0xC003);
    }
    #[test]
    fn test_flags() {
        let program:Vec<u8> =
            vec!(
                0xA9, 0xFF, // LDA #$FF
                0x85, 0x01, // STA $01 = 00
                0x24, 0x01, // BIT $01 = FF
                0xa9, 0x00, // LDA #$00
                0x38,       // SEC
                0x78,       // SEI
                0xf8,       // SED
                0x08,       // PHP
                0x68,       // PLA
            );
        let mut c = create_test_cpu(&program);
        run_clocks(&mut c, 9);
        assert_eq!(c.acc, 111);
    }
    #[test]
    fn test_adc() {
        let program:Vec<u8> =
            vec!(
                0xA9, 0x00, // LDA #$00
                0x69, 0x69, // ADC #$69
            );
        let mut c = create_test_cpu(&program);
        c.set_status_register_from_byte(0x6E);
        run_clocks(&mut c, 2);
        assert_eq!(c.status_register_byte(true), 0x2c);
    }
    #[test]
    fn test_bcc() {
        let program:Vec<u8> =
            vec!(0x90, 0x09,); // BCC #$09
        let mut c = create_test_cpu(&program);
        c.set_status_register_from_byte(0xf9);
        assert_eq!(c.carry, true);
        let pc = c.pc;
        c.clock();
        assert_eq!(c.pc - pc, 2); // Branch not taken
    }
    #[test]
    fn test_cmp() {
        let program:Vec<u8> =
            vec!(0xc9, 0x4d); // CMP #$4D
        let mut c = create_test_cpu(&program);
        c.acc = 0x4D;
        c.set_status_register_from_byte(0x27);
        c.clock();
        assert_eq!(c.status_register_byte(true), 0x27);
    }
    #[test]
    fn test_jsr() {
        let program:Vec<u8> =
            vec!(0x20, 0x03, 0xc0, // JSR $c003
                 0x68,             // PLA
            );
        let mut c = create_test_cpu(&program);
        run_clocks(&mut c, 2);
        assert_eq!(c.acc, 0x02);
    }
    #[test]
    fn test_lsr() {
        let program:Vec<u8> =
            vec!(0xa9, 0x01, // LDA #$01
                 0x4a,);     // LSR
        let mut c = create_test_cpu(&program);
        c.set_status_register_from_byte(0x65);
        run_clocks(&mut c, 2);
        assert_eq!(c.status_register_byte(true), 0x67);
    }
    #[test]
    fn test_asl() {
        let program:Vec<u8> =
            vec!(0xa9, 0x80, // LDA #$80
                 0xa);       // ASL
        let mut c = create_test_cpu(&program);
        c.is_tracing = true;
        c.set_status_register_from_byte(0xe5);
        run_clocks(&mut c, 2);
        assert_eq!(c.acc, 0);
        assert_eq!(c.status_register_byte(true), 0x67);
    }
    #[test]
    fn test_ror() {
        let program:Vec<u8> =
            vec!(0xa9, 0x55, // LDA #$55
                 0x6a);      // ROR
        let mut c = create_test_cpu(&program);
        c.set_status_register_from_byte(0x24);
        run_clocks(&mut c, 2);
        assert_eq!(c.acc, 0x2A);
        assert_eq!(c.status_register_byte(true), 0x25);
    }
}