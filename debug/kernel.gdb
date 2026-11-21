set pagination off
set confirm off
set disassembly-flavor intel
file target/x86_64-unknown-none/debug/leeloo

python
import gdb, time
for _ in range(150):
    try:
        gdb.execute("target remote :1234")
        break
    except gdb.error:
        time.sleep(0.2)
end

hbreak kmain
hbreak rust_begin_unwind

define regs
  monitor info registers
end
define tlb
  monitor info tlb
end
define pf
  p/x $cr2
end
