import re
import csv
from datetime import datetime

def extract_time_and_mem(log_file):
    time_sec = None
    mem_kb = None
    with open(log_file, 'r', errors='ignore') as f:
        content = f.read()
        
    time_match = re.search(r'Elapsed \(wall clock\) time \(h:mm:ss or m:ss\):\s*(\d+):([\d.]+)', content)
    mem_match = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', content)
    
    if time_match:
        mins = float(time_match.group(1))
        secs = float(time_match.group(2))
        time_sec = mins * 60 + secs
    
    if mem_match:
        mem_kb = int(mem_match.group(1))
    
    return time_sec, mem_kb

ex1_cpp_time, ex1_cpp_mem = extract_time_and_mem('results/ex1_cpp.log')
ex1_rs_time, ex1_rs_mem = extract_time_and_mem('results/ex1_rs.log')
ex2_cpp_time, ex2_cpp_mem = extract_time_and_mem('results/ex2_cpp.log')
ex2_rs_time, ex2_rs_mem = extract_time_and_mem('results/ex2_rs.log')

with open('results/summary.csv', 'w', newline='') as f:
    writer = csv.writer(f)
    writer.writerow(['example', 'tool', 'time_sec', 'mem_kb'])
    writer.writerow(['ex1_wgbs_se', 'bsmap_cpp', ex1_cpp_time, ex1_cpp_mem])
    writer.writerow(['ex1_wgbs_se', 'bsmaprs', ex1_rs_time, ex1_rs_mem])
    writer.writerow(['ex2_wgbs_pe', 'bsmap_cpp', ex2_cpp_time, ex2_cpp_mem])
    writer.writerow(['ex2_wgbs_pe', 'bsmaprs', ex2_rs_time, ex2_rs_mem])

print("Summary CSV 已更新")
