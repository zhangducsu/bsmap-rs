# 项目协作规则

1. 先理清需求再编码：需求模糊或信息不足时，先向用户确认边界和判断依据。
2. 保持极简实现：优先使用最简单直白的方案，避免过度设计。
3. 精准局部修改：只改必要代码，不随意重构或格式化无关代码。
4. 目标驱动开发：先定义完成标准，再迭代开发并主动验证。
5. 分清模型与代码职责：模型处理模糊判断；固定逻辑、路由、重试交给代码。
6. 严格控制 Token 预算：上下文临近阈值时主动总结并精简。
7. 风格统一不折中：代码风格或架构方案冲突时选定一种并说明理由。
8. 先研读再动手编写：修改前先阅读现有接口、工具函数和调用关系。
9. 测试对齐业务意图：测试必须覆盖核心业务逻辑，而不只验证能运行。
10. 关键节点设置检查点：多步骤任务每完成一个重要阶段复盘状态。
11. 遵从项目现有规范：个人审美服从仓库已有代码风格和目录约定。
12. 显性暴露失败与未完成部分：如实说明失败项、残缺项、跳过项和风险。
13. 目录树与命名规范约束：同类资源归集到固定目录，命名语义清晰。
14. 本地跑任何任务优先使用 WSL2 环境。
15. 总是用中文回复和写文档。
16. 访问服务器必须使用 MCP 工具；服务器写入和删除操作只允许在 Docker 环境中进行。
17. 持续维护踩坑记录：开发过程中一旦确认新的环境、工具链、Git、服务器或 benchmark 陷阱，必须在本文件追加“现象、原因、规避方式、验证方式”；不得只留在会话或临时报告中。

## 服务器连接规则

- 不再使用或配置直连容器端口 `12222` 作为服务器访问路径。
- 固定先通过 MCP hydrossh 连接宿主机 `rx2-huqi`：
  - host: `175.178.251.44`
  - port: `10096`
  - username: `huqi`
  - authMethod: `key`
  - privateKeyPath: `C:/Users/zhang_i5edc0/.ssh/id_rsa_rx2-huqi`
- 进入 Docker 时，从已连接的宿主机执行 `docker exec <container> ...`。
- 只读检查可以在宿主机执行，例如 `docker ps --format '{{.ID}} {{.Names}} {{.Status}} {{.Ports}}'`。
- 写入和删除必须放到容器内执行，例如 `docker exec <container> bash -lc '<command>'`。
- 当前已知容器名为 `vscode-ssh2`；执行前仍应先用只读 `docker ps` 确认容器 ID、名称和状态。

## 已知踩坑与规避

### 1. WSL2 与本地 Rust 工具链

- 执行本地任务前先运行 `wsl.exe --list --verbose`，确认发行版真实可用。
- 若 WSL 返回“无已安装发行版”，应标记为本机环境阻塞，不得写成代码编译失败，也不得未经用户确认自行安装发行版。
- 当前 Windows GNU Rust 可能因缺少 `dlltool.exe` 失败，MSVC 目标也可能缺少 Windows SDK/系统链接库。这类失败发生在项目代码编译前，不能据此判断代码回归。
- WSL2 不可用时，优先用同一提交的 Docker Linux 工具链完成 `cargo check/test/build` 和回归测试，并在报告中明确测试环境替代。

### 2. 命令必须 fail-fast

- 不要用 PowerShell 分号串联多个关键验证后，只看最后一个命令的退出码；前面的 `cargo fmt`、测试或构建失败可能被后续成功命令掩盖。
- PowerShell 中应在每个关键命令后检查 `$LASTEXITCODE`；Bash 使用 `set -euo pipefail`。
- 仓库当前存在既有 rustfmt 漂移，`cargo fmt --all -- --check` 会报告大量非本任务文件。只审查本次改动文件，并用 `git diff --check` 防止空白错误，不得顺手格式化全仓库。

### 3. Linked worktree 的 Git 锁权限

- linked worktree 的实际 Git 元数据位于主仓库 `.git/worktrees/<name>`，不是 worktree 内的 `.git` 文件。
- 若出现 `index.lock: Permission denied`，先确认是沙箱/ACL 权限问题，不要反复重试，也不要声称已提交。
- 权限无法恢复时，可在项目目录下建立临时 integration clone：只复制已审查的明确文件、检查 `git diff --cached --stat`、提交并推送同一 P13 分支。临时 clone 不得发展独立代码，任务结束后应清理。
- 不使用 `git reset --hard` 处理主工作区或用户改动；Docker 内专用临时 checkout 只有在计划明确要求时才可 reset 到远端提交。

### 4. GitHub 推送与远端确认

- Windows HTTPS push 若报 `schannel: AcquireCredentialsHandle failed: SEC_E_NO_CREDENTIALS`，使用单次命令：

  ```powershell
  git -c http.sslBackend=openssl -c credential.helper= push origin <branch>
  ```

- remote URL 可能含凭据，禁止在日志、报告或回复中输出完整 URL。
- push 输出已显示远端更新后，即使随后本地 remote-tracking ref 因锁权限更新失败，也不要盲目重复 push；先通过 GitHub 或服务器 fetch 验证远端 commit。
- 当前 GitHub App 可能只有读权限，Git Data API 创建 blob/tree 会返回 403；它不能替代本地 `git push`。

### 5. Docker 内 GitHub 同步

- Docker 内 `git fetch` 偶发 `curl 16 Error in the HTTP2 framing layer` 时，只对该次 fetch 使用：

  ```bash
  git -c http.version=HTTP/1.1 fetch origin <branch>
  ```

- fetch 后必须核对 `git rev-parse HEAD` 或目标 commit，再开始编译和 benchmark。
- MCP 命令 timeout 后，Docker 子进程可能仍继续运行。重跑前必须用 `ps` 和结果目录检查进程；禁止直接启动第二份重复 benchmark。

### 6. Sparse checkout 与 C++ BSMAP

- 服务器临时仓库可能启用了 sparse checkout，只展开 `bsmap-rs`。缺少 `bsmap-original` 时先检查：

  ```bash
  git sparse-checkout list
  git ls-files bsmap-original
  ```

- `bsmap-original` 是普通 tracked 目录，不是子模块；使用 `git sparse-checkout add bsmap-original`。不要运行全仓库 `git submodule update --init --recursive`，因为 `bsmap-rs/tools/BSBolt` 等 gitlink 缺少可用 `.gitmodules` URL，会直接失败。
- 仓库中的 C++ `bsmap` 可能没有 executable bit。只能在 Docker benchmark 前临时 `chmod +x`，结束后恢复；也可通过 runner 的 `CPP_BINARY` 指向已验证二进制。
- C++ PE 已知可能退出 134。运行前在 Docker 设置 `ulimit -c 0`；否则会在 checkout 生成约 2 GB core 文件。已有 core 只能在 Docker 内确认路径后删除。

### 7. Benchmark 可复现性

- mm10 runner 使用：

  ```bash
  CPP_BINARY=/workspace/03_project/bsmap-2.90/bsmap \
    bash bsmap-rs/benchmark/p13/run_docker_mm10.sh \
    /tmp/p13_codex_github/repo /tmp/p13_codex_github/runs
  ```

- benchmark 前确认 `git status --porcelain` 为空。`python -m py_compile` 会生成 `__pycache__`，只能在 Docker 内清理后再记录 `repo_dirty`。
- 强制重建 RRBS `.bsi` 时，只删除结果目录临时 reference symlink 对应的索引，不得删除原始数据目录的索引。
- C++ 多重命中选择具有随机性；逐步对比时应先确认双方随机种子参数并显式固定，同时把参数写入 metadata。未固定随机种子时，不得把 secondary FLAG、Top RNAME 的小幅波动误判为确定性回归。
- SAM 对比必须把 FASTQ/SAM QNAME 截断到第一个 ASCII 空白；PE 还要按 FLAG 64/128 区分 mate。比较至少包含 RNAME、POS、FLAG、NM。
- 每个结果必须记录 commit、binary/input SHA256、完整命令、退出码、wall/user/sys、CPU、max RSS、SAM 统计和原始结果路径。

### 8. 踩坑记录维护格式

新增记录时使用以下最小格式，确认后当轮立即更新：

```markdown
### <编号>. <标题>

- 现象：
- 原因：
- 规避：
- 验证：
```

只记录已复现或有源码/日志证据的事实；猜测放在报告“待验证项”，不得写成固定规则。

### 9. OneDrive 超长 Codex checkpoint ref

- 现象：`git fetch`、`git gc` 或 geometric repack 报 `fatal: bad object refs/codex/turn-diffs/checkpoints/...`；`git update-ref -d` 又因 `Filename too long` 无法创建锁文件。
- 原因：仓库位于 OneDrive 长路径下，Codex checkpoint ref 的完整路径超过 Windows 普通路径限制；文件可能存在，但 PowerShell 普通路径 API 无法读取或删除。
- 规避：先确认目标严格位于 `.git/refs/codex/turn-diffs/checkpoints/`，并用 `git cat-file -e` 证明目标对象不存在。仅对这个损坏 checkpoint 文件使用 `\\?\` 长路径 API 精确删除；禁止递归删除 `refs/codex`，禁止触碰 `refs/heads`、`refs/remotes` 或 tags。
- 验证：删除后重新执行 `git fetch origin <branch>`，并核对 `git rev-parse origin/<branch>` 与 GitHub 目标 commit 一致。

### 10. Docker partial clone 的 lazy blob fetch

- 现象：服务器 `git fetch` 已更新远端 ref，但 `git reset --hard origin/<branch>` 长时间停在 `git-remote-https`，HEAD 不移动，worktree 出现部分删除/修改；MCP 超时后进程仍可能继续持有 `.git/index.lock`。
- 原因：Docker 临时仓库是 `--filter=blob:none` 的 partial clone，reset 新提交时触发按需 blob 下载；旧 Git 版本不支持 `git fetch --refetch`，网络异常会让 reset 半途失败。
- 规避：先等待并确认原 reset 进程退出、`index.lock` 消失，不得并发重跑。若同一问题连续复现，保留旧 checkout，在 Docker 内通过 GitHub 新建非 partial 的单分支完整 clone，并核对 commit；后续 build/benchmark 使用新路径。
- 验证：完整 clone 中 `git rev-parse --short HEAD` 等于目标提交，`git status --porcelain` 为空，并能直接读取本次变更文件而不启动 promisor fetch。

### 11. Docker benchmark 磁盘与 core 文件

- 现象：runner 写 `exit_code.txt` 或 SAM 时出现 `No space left on device`；`df -h` 显示 Docker overlay 100%。
- 原因：C++ PE 退出 134 可能在 runner 工作目录的父目录生成多个约 2.1 GB core；多个 checkout 的 Cargo `target` 又各占约 4 至 5 GB。
- 规避：每轮前在 Docker 内检查 `df -h` 和 `du -sh`。设置 `ulimit -c 0`；定期删除已确认的 core 和不再使用 checkout 的可重建 `target`。不得为了腾空间删除尚未汇总的 summary/metadata/raw result。
- 验证：清理后 overlay 必须有足够余量；空间不足导致的半成品 run 目录应在 Docker 内删除后完整重跑，不得复用。

### 12. WSL2 登录环境与本地 mm10 存储

- 现象：`wsl.exe -d Ubuntu -- cargo ...` 报 `cargo: command not found`，但 `wsl.exe -d Ubuntu -- bash -lic 'cargo ...'` 可正常运行；同时 WSL 内 `df -h /` 显示约 917 GB 可用空间，而承载 `ext4.vhdx` 的 C 盘实际只剩约 39 GB。
- 原因：非登录 shell 不一定加载 `$HOME/.cargo/env`；WSL 根文件系统显示的是 VHDX 逻辑上限，不代表宿主物理盘的真实剩余空间。本机 Ubuntu VHDX 位于 C 盘。
- 规避：本地 Rust 命令使用 Ubuntu 登录 shell，或显式设置 Cargo PATH。容量判断必须同时检查 WSL `df`、Windows 物理盘和 `HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss` 的 `BasePath`。P13 mm10 输入与结果统一放在 D 盘，不放入仓库、OneDrive 或 WSL 根盘。
- 验证：2026-06-21 本机 WSL2 使用 16 个逻辑 CPU、15 GiB 内存和 4 GiB swap，`cargo check -p bsmap` 通过；既有 Rust mm10 峰值 RSS 约 5.34 GB，硬件余量足够。后续 P13 mm10 10K benchmark 在本地 WSL2 执行，不再使用服务器计算；正式运行前必须核对 reference/R1/R2 SHA256 与 P13 报告一致。

### 13. 本地 benchmark 的 Git worktree、文件名映射与陈旧二进制

- 现象：WSL Git 无法把 linked worktree 识别为 checkout；在 D 盘 clone 后，仓库中的特殊 tracked 文件名因 NTFS/DrvFS 映射持续显示删除/新增；`/tmp` clone 又可能在 WSL 会话间消失。首次本地 mm10 run 虽记录 `b501aac`，但复用了旧 `target/release/bsmap`，SAM 分布回退到旧结果。
- 原因：linked worktree 的 `.git` 文件保存 Windows `C:/...` gitdir，WSL Git 不能直接解析；NTFS 不支持部分 Linux 文件名，DrvFS 会映射为私用区字符；`/tmp` 不是持久工作区；Git commit 与现存 release binary 没有自动绑定关系。
- 规避：P13 本地正式 benchmark 使用 WSL ext4 home 下的干净 detached clone 记录 commit 和 dirty 状态，输入与结果仍放 D 盘。每轮必须在当前源码执行 `cargo build --release -p bsmap`，记录 binary SHA256 后再运行。不得仅凭文件存在或时间戳认定 binary 对应 HEAD。
- 验证：正式 run `20260621T142726Z-1511` 的 clone 为 `repo_dirty=false`、commit 为 `b501aac`，新 Rust binary SHA256 为 `da6d07d517d9a13a20478673ef6fbfb4e5f19e34e3b8c4d17815906f67dbd27d`；Rust SE Top1 chr1 11.31%，复现服务器同提交结果。旧 binary SHA256 `10406863...` 的 17.27% 结果仅保留为无效诊断。

### 14. C++ 随机数常量与 RRBS logical bucket

- 现象：Rust/C++ SE mapped 集合和大部分候选集合已经相同，但固定 `-S 1` 后 secondary 坐标一致率仍只有约 82.6%。
- 原因：Rust `myrand` 的第二个乘数抄错一位；同时 reusable RRBS index 始终包含 BSC cross-chain hits，而 C++ SE 默认 bucket 不包含这些 hit。在完整 bucket 上先取模、再过滤 BSC，不能得到 C++ 的随机选择。
- 规避：为 RNG 增加由 C++ 二进制生成的固定向量测试，禁止仅用“Rust 自身可重复”作为等价证据。SE 必须先构造排除 `RRBS_BSC_FLAG` 的 logical bucket，再计算随机 modulus；PE 或 `-n 1` 才使用完整 bucket。
- 验证：`myrand(4610, 1, 0) == 3299322595`；提交 `d82ce8d` 后 mm10 2,423 条 SE records 的 RNAME/POS/FLAG/strand 全部与 C++ 一致。

### 15. C++ 默认 N mismatch 语义

- 现象：mm10 SE 2,423 条记录只剩 1 条 NM 不一致，同一 QNAME、RNAME、POS 和 FLAG 下 C++ 为 NM 0、Rust 为 NM 1。
- 原因：C++ 默认 `N_mis=0`，query mask 会排除 N，只有显式 `-N` 才把 N 加入 mismatch；Rust 虽清除了 N 对应 mask，仍无条件以 `n_count` 初始化 mismatch 总数。
- 规避：当前 Rust CLI 未实现 `-N` 时，scalar、SIMD 和 fallback 路径均从 0 开始 mismatch 计数；若以后实现 `-N`，必须显式传递该选项，不能恢复无条件累加。
- 验证：单测使用带 masked N 的 query，scalar/SIMD 均返回 0；提交 `0edefff` 后 mm10 NM 达到 2,423/2,423 一致。

### 16. 遗留 rustfmt 漂移会放大局部补丁

- 现象：对 6 个本次修改文件直接执行 `rustfmt`，功能改动约 200 行却生成 738 insertions、447 deletions，包含大量无关旧代码重排。
- 原因：仓库部分 Rust 文件长期未与当前 rustfmt 版本对齐；即使只指定文件，rustfmt 仍会格式化整个文件，而不是只格式化新 hunk。
- 规避：局部任务不直接对遗留大文件运行 rustfmt。手工保持周边风格，用 `git diff --check`、编译和测试验证；误执行后必须先精确移除本轮格式噪声，再重放功能补丁，禁止把全文件格式变化带入提交。
- 验证：P13 ZP/ZL 最终提交 `365124d` 仅包含 6 个必要文件、216 insertions、33 deletions；完整 `cargo check/test/build` 通过。

### 17. 外层工具超时不等于 WSL benchmark 已停止

- 现象：本地 mm10 runner 的外层命令返回 timeout，但 WSL 中 `run_docker_mm10.sh` 和 `bsmap` 仍继续运行并写入结果目录。
- 原因：外层 PowerShell/工具只停止等待，未必终止 WSL 内已经派生的进程；立即重跑会并发构建索引并争用内存、I/O 和结果资源。
- 规避：任何 timeout 后先在 WSL 执行 `ps -eo pid,ppid,etime,cmd`，并检查最新 run 目录、`exit_code.txt` 与 `summary.json`。只有确认原 runner 已退出且结果确实不完整时才能重跑。
- 验证：run `20260621T162345Z-2952` 在外层 timeout 后仍完成 Rust SE/PE；检查进程后未启动重复 Rust benchmark。该 run 因 C++ executable bit 返回 126，失败原因与 timeout 无关。

### 18. Windows checkout 的 shell 脚本换行符

- 现象：P13 worktree 中可运行的 benchmark 脚本，在 `main-work` fast-forward 后执行时报 `set: pipefail\r: invalid option name`。
- 原因：仓库设置 `core.autocrlf=true`，新 checkout 把 tracked shell 脚本转换成 CRLF；原 P13 worktree 中脚本创建时保持 LF，所以合并前验证没有暴露该差异。
- 规避：仓库根目录使用 `.gitattributes` 固定 `*.sh text eol=lf` 和 `*.py text eol=lf`。新增跨平台脚本后必须在新的 Windows checkout 或干净 worktree 中实际通过 WSL 执行，不能只在创建脚本的原 worktree 验证。
- 验证：集成提交 `1422653` 加入换行规则；重新规范化工作区文件后，`run_local_ex1_ex2.sh postmerge_main` 正常完成，example1 Rust/C++ 均为 66,120 mapped，坐标一致率 98.83%。


### 19. Rust standalone index 与 C++ 单样本计时边界

- 现象：把 Rust 首次自动建索引耗时并入 align，会把一次性成本重复算到每个样本；反过来把 C++ 内部建索引从普通 invocation 中人工扣除，又会构造不存在的 C++ 使用方式。
- 原因：Rust P14 提供独立 `bsmap index` 和可复用 `.bsi`，C++ BSMAP 2.90 没有等价的可复用 standalone index 接口。
- 规避：Rust 报告必须分成 standalone index、warm process 和 alignment core；Rust/C++ 单样本比较只用 Rust 已有索引的 warm process 对 C++ normal invocation。Rust standalone index 另表记录，绝不加回每个样本。
- 验证：P14 mm10 standalone index 为 49.14 秒；Rust warm SE 中位 12.39 秒；C++ normal invocation 为 121.98 秒，三者在报告中分栏。

### 20. WSL 无法解析 Windows linked-worktree 的 gitdir

- 现象：源码和 Cargo 在 WSL 可用，但 `git -C <worktree> rev-parse HEAD` 报不是 Git 仓库。
- 原因：linked worktree 的 `.git` 文件保存 Windows `C:/...` gitdir；WSL Git 不会自动把该内部路径转换为 `/mnt/c/...`。
- 规避：linked worktree 内的 Git 操作使用 Windows Git；WSL benchmark runner 通过 `GIT_COMMIT=<sha>` 显式记录提交，或在 WSL ext4 clone 中运行。不得把 commit 留空或伪造。
- 验证：P14 runner 已支持 `GIT_COMMIT`；缺少可解析 Git checkout 且未提供该变量时 fail-fast。

### 21. PowerShell 调用 Bash 时的变量与参数边界

- 现象：PowerShell here-string 通过未加引号的 `bash -lc $script` 传递后，Bash 中的 `$OUT/$i` 丢失，路径退化为 `/se-warm-.stdout`。
- 原因：native command 参数在 PowerShell 到 WSL 的边界被重新拆分；嵌套的 `$`、引号和管道还可能先被 PowerShell 解释。
- 规避：正式 benchmark 优先调用仓库脚本；临时命令使用一个完整的单引号 Bash command，或直接展开绝对路径，不在两层 shell 间传未转义变量。
- 验证：错误命令在写结果前退出；改为显式绝对路径后 mm10 warm 三轮均正常生成独立 time/SAM 文件。

### 22. v7 raw-section 格式必须有布局标记和边界校验

- 现象：早期 P14 v7 仍用 bincode 保存 index arrays；若只按 version=7 接受，会把两种不兼容布局当成同一格式。
- 原因：版本号先被用于 metadata compatibility，随后才完成 raw-section 布局。
- 规避：v7 header 必须包含 `RAWSECT1` marker、section offset/count 和文件边界校验；缺 marker 的旧 v7 明确拒绝并重建。不得仅改版本号伪造旧格式 fixture。
- 验证：单测覆盖 raw mmap round-trip、真实 v6 fixture、旧缓存拒绝和 section 越过 EOF。

### 23. mmap advice 不能对 WGBS/RRBS 一刀切

- 现象：mm10 RRBS 使用默认 mmap advice 时 warm RSS 约 1.60 GiB；全局改为 `MADV_RANDOM` 后 RRBS 降至约 1.25 GiB，但 1 Mb WGBS wall time明显变慢。
- 原因：RRBS 候选对大索引是随机页访问，默认顺序预读扩大驻留集；小 WGBS reference/index 则能从默认预读获益。
- 规避：仅 RRBS v7 mmap 使用 `MADV_RANDOM`，WGBS 保持系统默认。任何 advice 变化都必须同时复测 wall、major faults、RSS 和完整 SAM。
- 验证：P14 最终 mm10 SE 最坏 RSS 为 1,309,692 KiB（1.249 GiB），WGBS example1 warm p8 为 2.00 秒且 66,120 条完全一致。

### 24. GNU time 的 PE 失败判定与 RSS 单位

- 现象：C++ PE 的 time 文件可同时出现 `Command terminated by signal 6` 和末尾 `Exit status: 0`；仅 grep 最后一行会误判成功。`Maximum resident set size (kbytes)` 也常被直接当十进制 KB。
- 原因：signal termination 的 GNU time 展示不能只靠最后一个字段判断；Linux time 的 RSS 数值按 KiB 换算更准确。
- 规避：C++ PE 同时检查 signal 行、stderr、SAM 大小和外层退出码；运行前 `ulimit -c 0`。RSS 报告保留原始 KiB，并用 `KiB / 1,048,576` 给出 GiB。
- 验证：WGBS example2 C++ PE 记录 signal 6、buffer overflow、0-byte SAM；mm10 C++ PE 记录 signal 6/134，均未伪装成有效对照。

### 25. P16 优化候选必须同环境覆盖 WGBS 与 RRBS

- 现象：ThinLTO、`codegen-units=1`、`panic=abort`、PE 临时对象复用等候选在 WGBS 小样本上看似变快，但同环境 mm10 RRBS SE/PE 会变慢，甚至 PE wall 回退约 9% 到 12%。
- 原因：BSMAP-rs 的 WGBS 小 reference、RRBS mmap 大索引、PE pairing 和 SAM 输出瓶颈不同；单一 workload 的收益不能代表全局收益。
- 规避：任何默认性能优化必须用同一脚本、同一机器、同一 binary 构建口径比较 P15/P16，至少覆盖 WGBS example1/example2 和 mm10 RRBS SE/PE 10K。Rust standalone index 必须单独计时，不得混入 align wall time。
- 验证：P16 保留 `benchmark/p16/run_short_validation.sh`；负收益候选写入 `benchmark/P16_Engineering_Optimization_Report.md`，未达标优化必须撤回，不得只凭直觉保留。

### 25. 托管沙箱可能隐藏 WSL 发行版

- 现象：托管沙箱内执行 `wsl.exe --list --verbose` 返回“没有已安装发行版”，同一会话在沙箱外执行却能列出 Ubuntu 和 `docker-desktop`，均为 WSL2。
- 原因：沙箱进程看不到宿主用户的完整 WSL 注册状态；该结果不等于发行版被卸载。
- 规避：沙箱内出现空列表时，不得直接判定 WSL 环境丢失。先申请只读的沙箱外 `wsl.exe --list --verbose` 检查；后续 WSL 编译和 benchmark 也在获批的宿主环境中执行。
- 验证：2026-06-22 沙箱外列出 Ubuntu 与 `docker-desktop`；随后 Ubuntu 登录 shell 中 `cargo build --release -p bsmap` 成功。

### 26. PE 输入 FIFO 的打开顺序会死锁

- 现象：单个 producer 依次打开 R1、R2 FIFO 且等待两者都打开后才写数据时，PE runner 中 producer、`bsmap` 和 GNU time 长时间全部处于等待状态。
- 原因：`FastqReader` 打开 R1 后会先读取格式字节，再打开 R2，而且后续也可能按 mate 分批读取。单 writer 会阻塞在打开 R2；按 pair 向两个 bounded writer 锁步投递时，R2 管道/队列先填满又会反向阻塞 R1，形成第二种死锁。
- 规避：R1、R2 必须由两个完全独立的常数内存 producer 流式输出，使用相同 repeat count，结束后核对 records 数。PE 规模测试使用 `TARGET_SOURCE_BYTES` 或 `REPEATS`；不能用需要逐 pair 联动停止的 `TARGET_EMITTED_BYTES`。
- 验证：POSIX FIFO 单测按“读取完 R1 后才打开并读取 R2”的顺序执行，两个 mate 均完整输出且线程正常退出；example2 smoke run `20260622T145010Z.meNSxk` 在 1.17 秒完成，66,958 条 SAM 的 SHA256 与 P15 Phase 1 完全一致。

### 27. RRBS hit section 预读会用 RSS 和尾延迟换取部分缺页下降

- 现象：RRBS v9 对 `rrbs_hits` 恢复默认 `MADV_NORMAL`，SE major faults 中位从 207,813 降到约 149,159，但 wall 从 8.61 秒退化到 9.11 秒，最坏 RSS 从 829,560 KiB 增至 927,984 KiB；改为 `MADV_WILLNEED` 后 wall 中位进一步退化到 9.93 秒，RSS 仍约 928 MiB。
- 原因：393 MB hit section 的内核预读减少了显式 major fault 次数，却扩大驻留集并引入额外 I/O/缓存压力；在本机 WSL2 + D 盘 DrvFS 上，缺页计数下降不等于端到端 wall 改善。
- 规避：RRBS v9 暂时保持全 RRBS mmap 的 `MADV_RANDOM` 基线。任何 section advice 或 readahead 实验必须同时比较三轮中位 wall、最坏 RSS、major faults 和 SAM SHA；不得只因 faults 下降就保留。
- 验证：`D:/BSMAP/benchmark-results/p15/phase3-section-advice` 与 `phase3-willneed` 均保持 2,423 条和 SHA `420e34a3...`，但性能门槛失败，代码已精确回退。

### 28. PowerShell 到 WSL 的含空格环境变量会被拆分

- 现象：从 PowerShell 调用 WSL 时写 `env THREAD_MATRIX="1 2 4 8 16" bash ...`，脚本没有执行线程矩阵，反而打印了环境变量列表；跨边界传 `grep -E "a|b"`、Bash here-doc 或嵌套 Python 字符串时也会出现 pattern 被当作管道、变量丢失或引号被吃掉。
- 原因：PowerShell、`wsl.exe` 和 Bash 的多层参数边界会重新拆分带空格、`|`、`$`、here-doc 和嵌套引号的参数；外层看似已经加引号，进入 Linux 侧后仍可能变成不同 argv 或不同 shell 语法。
- 规避：正式 benchmark 不通过一行命令传递带空格的 env 值或复杂脚本片段；优先使用仓库脚本、脚本默认矩阵、临时脚本文件或简单 `cat/ps` 轮询。需要传递时用完整单引号 Bash command，并在 Linux 侧用 `printf '%q\n' "$VAR"` 或 `bash -n` 自检。
- 验证：去掉跨边界的 `THREAD_MATRIX="1 2 4 8 16"` 后，`run_thread_matrix.sh` 默认生成 p1/p2/p4/p8/p16 共 15 个 run，并输出 `thread_matrix.json`；RRBS/WGBS 长测改用简单 `env ... bash run_stream_scale.sh` 后正常启动。

### 29. DrvFS 会放大 mm10 RRBS 索引与比对的缺页和 wall time

- 现象：同一 v10 mm10 RRBS 索引在 D 盘 DrvFS 构建约 64.00 秒，在 WSL ext4 forward-only 构建约 33.86 秒；RRBS alignment 在 ext4 上 major faults 接近 50 到 60，而 DrvFS v9 约 20 万级。
- 原因：DrvFS 通过 Windows 文件系统桥接大 mmap/random access，页缓存、缺页和 metadata 行为与原生 Linux ext4 差异很大；这会掩盖 Rust 索引布局本身的真实收益。
- 规避：正式 Linux/部署性能数字使用 WSL ext4 或服务器 Docker ext4/overlay；D 盘 DrvFS 结果只能作为 Windows 文件系统限制记录。大型输入和结果可留在 D 盘，但会被频繁 mmap 的 `.bsi` 和 reference 应复制/硬链接到 ext4。
- 验证：v10 forward-only ext4 index SHA 与 DrvFS v10 index SHA 均为 `d7afbc84...`，说明数据等价；性能差异来自文件系统路径而非索引内容。

### 30. P16 runner 的 repo root 与 run-id 参数边界

- 现象：从 `bsmap-rs` 子目录执行 `run_short_validation.sh . ...` 时，脚本会查找 `bsmap-rs/bsmap-rs/target/release/bsmap` 并失败；从 PowerShell 调用 WSL 时使用 Bash 变量 `$RUN_ID` 作为输出目录，变量可能在跨 shell 边界被吃掉，结果写入 `D:/BSMAP/benchmark-results/p16/` 根目录。
- 原因：P16 runner 的第一个参数语义是仓库根目录，不是 `bsmap-rs` crate 目录；PowerShell、`wsl.exe`、Bash 多层参数边界会提前解释或丢失 `$` 变量。
- 规避：正式运行时从仓库根目录调用：`bash bsmap-rs/benchmark/p16/run_short_validation.sh . <绝对结果目录>`。结果目录使用显式完整路径，不在一行 PowerShell 命令里依赖 `$RUN_ID` 拼接。
- 验证：显式执行 `bash bsmap-rs/benchmark/p16/run_short_validation.sh . /mnt/d/BSMAP/benchmark-results/p16/sam-direct-warm-20260623T072000Z` 正常完成，example1 与 RRBS SE 均达到 Rust/C++ 完整逐行一致。

### 31. RRBS 服务器 warm align 不能删除 Rust `.bsi`

- 现象：SSH 服务器复测中，Rust 10K PE wall time 约 49 秒，但日志显示其中包含 v10 RRBS `.bsi` 重建和约 46 秒参考/索引加载；该数字不能作为 Rust/C++ 单样本 align 对比。
- 原因：旧 Docker runner 在每个 case 前删除临时 reference symlink 对应的 `.bsi`，Rust align 触发自动建索引；同时 C++ 没有等价 standalone index，导致计时口径混乱。
- 规避：服务器 RRBS warm align runner 必须要求 `REFERENCE.bsi` 预先存在，运行前后记录并校验 index SHA；Rust standalone index 单独计时，绝不混入 align wall。需要强制重建索引时必须作为独立 case 记录。
- 验证：SSH1 runner 写入 `standalone_index_included=false`，并在结束时比较 `index_sha256_before` 与 `index_sha256_after`；SHA 不一致时 fail-fast。

### 31. 同一 linked worktree 的 Git index 操作不能并行

- 现象：在同一个 linked worktree 上并行执行 `git status` 和 `git merge --ff-only`，merge 报 `index.lock: File exists`。
- 原因：`status`、`merge`、`add`、`commit` 等命令都可能读写或刷新同一个 worktree index；并行执行会争用 `.git/worktrees/<name>/index.lock`。
- 规避：同一 worktree 内所有会触碰 index 的 Git 命令必须串行执行。可以并行只读文件读取、`git log`、`git diff` 等不刷新 index 的查询；不确定时按会写 index 处理。
- 验证：确认没有残留 Git 进程且 lock 自动消失后，串行执行 `git status --short --branch` 再执行 `git merge --ff-only codex/p16-engineering-performance`，main 成功 fast-forward 到 `29daa8f`。

### 32. OneDrive linked worktree 删除可能留下半移除状态

- 现象：`git worktree remove` 对 `.claude/worktrees/<name>` 报 `Permission denied`，随后该 worktree 可能已从 `git worktree list` 消失，但 `.claude/worktrees/<name>` 和 `.git/worktrees/<name>` 目录仍残留。
- 原因：OneDrive/Windows 文件属性或同步状态会阻止 Git 删除工作区目录和 linked worktree 元数据；Git 命令可能已经完成 unregister，但文件系统删除失败。
- 规避：先确认目标 worktree 干净且不含用户未跟踪文件，再用 `git worktree list`、`Test-Path` 和 `Resolve-Path` 确认残留目录严格位于当前仓库的 `.claude/worktrees/` 与 `.git/worktrees/` 下。只对确认的单个残留目录使用 PowerShell `Remove-Item -LiteralPath <path> -Recurse -Force`；禁止对 `.claude/worktrees` 或 `.git/worktrees` 做通配递归删除。
- 验证：P14/P15/P16 baseline/P17/main-work 残留目录按精确路径删除后，`git worktree prune` 和 `git worktree list --porcelain` 只剩主工作区、干净 main 工作区和保留的 P13 工作区。

### 33. Windows archive/tar 与异常文件名会阻塞基线导出

- 现象：在 Windows/PowerShell 中用 `git archive <commit> | tar -x` 导出 baseline 时，`tar` 报 `Unrecognized archive format`；导出全仓库或整个 `bsmap-rs` 时又可能遇到历史异常文件名导致路径无法创建。
- 原因：PowerShell/native pipe、Windows `tar` 和 NTFS 路径规则会影响二进制 archive 流和部分 Linux 风格文件名；该问题发生在基线工作区导出阶段，不代表源码或 Cargo 构建失败。
- 规避：不要在 Windows 上为本仓库做全量 archive 解包。需要构建 Rust baseline 时，只导出构建必需路径，例如 `bsmap-rs/Cargo.toml`、`bsmap-rs/Cargo.lock`、`bsmap-rs/bsmap`、`bsmap-rs/methratio`、`bsmap-rs/bsp2sam`；或在 WSL ext4 中使用干净 clone/sparse checkout。
- 验证：S1 baseline `9a4f7ca` 使用构建必需路径导出到 `D:/BSMAP/scratch/s1-baseline-9a4f7ca` 后，`cargo build --release -p bsmap` 成功，baseline binary SHA256 为 `96ac6f102b77245444a40a802132a46148a69a90c4030ecf8ea769341c088186`。

### 34. 托管 PowerShell 可能没有可用 Python 解释器

- 现象：`python -m py_compile ...` 和 `python3 -m py_compile ...` 直接报命令不存在；`py -3` 存在但返回 `No installed Python found!`。
- 原因：Windows Python launcher 可能已安装，但托管 sandbox 的 PATH 中没有真实 Python 解释器；这与 benchmark 脚本语法是否正确无关。
- 规避：不要把该错误写成脚本回归。优先在 WSL2 登录 shell 或服务器 Docker 内执行 `python3 -m py_compile benchmark/ssh1/*.py`；本地 sandbox 只能记录为环境限制。
- 验证：先用 `Get-Command python,python3,py -ErrorAction SilentlyContinue` 区分 launcher 与解释器，再在可用 Linux 环境中补跑 py_compile。

### 35. PowerShell here-string 通过 SSH 传 Bash 脚本会污染换行

- 现象：从 PowerShell 用 here-string 直接管道到 `ssh ... docker exec -i ... bash -s` 后，容器内 `cargo build --release -p bsmap` 报 `invalid character '\r' in package name: bsmap\r`；检查仓库 `Cargo.toml` 没有 CR 字符。
- 原因：PowerShell here-string/native pipe 会把 CRLF 作为脚本换行传给 Bash；Bash 不把 `\r` 当作行结束的一部分剥离，导致行尾参数变成 `bsmap\r`。
- 规避：跨 PowerShell -> SSH -> Docker 传复杂脚本时，先把脚本文本转换成 LF，再 base64 编码，通过远端 `printf '%s' <base64> | base64 -d | docker exec -i <container> bash` 执行。简单命令可用单层明确引号；避免在多层 shell 间直接传 here-doc、未转义 `$`、管道和带空格 env。
- 验证：用 base64/LF 方式重新执行同一容器 checkout 的 `cargo build --release -p bsmap`，构建通过；错误未再出现。

### 36. RRBS count-heavy profiling 不能直接跑 full 数据

- 现象：`BSMAP_PROFILE_RRBS=1` 的 10K SE 可完成并输出候选计数，但同样设置跑 full SE 时，任务明显被 atomic 热路径计数放大，约 1 分 40 秒仅写出约 23 MB SAM，无法作为正式性能基线。
- 原因：`BSMAP_PROFILE_RRBS=1` 会在 RRBS segment、candidate、mismatch 和 accepted hit 热路径上做 atomic 计数；10K 诊断中已记录约 1.03 亿 raw candidates 和 2,003 万 mismatch 调用，full 数据会把该开销线性放大。
- 规避：full run 只使用 `BSMAP_PROFILE_RRBS=stage` 或 `SSH1_PROFILE_RRBS=stage` 输出 read/prepare/align/write 阶段耗时；详细 candidate/mismatch/hit 计数只在 10K 或抽样数据上启用。正式 Rust/C++ wall time 对比必须使用 `SSH1_PROFILE_RRBS=0` 的 warm run。
- 验证：10K `SSH1_PROFILE_RRBS=1` 保持 Rust/C++ 2,423 条记录完全一致并输出完整 profile；full `BSMAP_PROFILE_RRBS=1` 中止后不纳入报告，改由 stage-only 方案继续定位。

### 37. Docker 内 GitHub fetch 超时时可用 Git bundle 增量同步

- 现象：本地已成功 push 到 GitHub，但 Docker 内 `git -c http.version=HTTP/1.1 fetch origin <branch>` 仍可能报 `curl 28 Failed to connect to github.com port 443`，导致 sparse checkout 无法更新。
- 原因：容器到 GitHub 的出站网络可能临时不可达；这与本地 push 成功、代码是否存在于远端无关。
- 规避：若容器 checkout 已有旧基线提交，可在本地生成只包含增量对象的 bundle，例如 `git bundle create <tmp.bundle> <branch> ^<old_commit>`；用 SSH 将 base64 后的 bundle 流入 Docker 内 `/tmp`，再在容器里执行 `git fetch /tmp/<bundle> <branch>:refs/remotes/origin/<branch>` 和 `git reset --hard origin/<branch>`。bundle 写入、fetch、reset 都发生在 Docker 内；不要把 bundle 写到宿主机项目目录。
- 验证：`d761fe4` 通过 4 KB bundle 从本地同步到 `/tmp/ssh1_sparse_20260627T153127Z_68025/repo`，容器内 `git rev-parse --short HEAD` 为 `d761fe4`，`git status --porcelain` 行数为 0，随后 `cargo build --release -p bsmap` 通过。

### 38. PowerShell 原始二进制流不能直接当作可靠 SSH 传输

- 现象：用 PowerShell `Get-Content -Encoding Byte | ssh ... docker exec ... cat > /tmp/<bundle>` 传 Git bundle 后，容器内文件大小从本地 3,317 bytes 膨胀到 15,148 bytes，`git bundle verify` 失败；改用 PowerShell 管道传 base64 文本时也出现 `base64: invalid input`。
- 原因：PowerShell 到 native command 的管道会按对象或文本语义重组数据，不保证原始二进制 byte-for-byte 传输；多层 SSH/Docker 管道还会放大换行和编码问题。
- 规避：GitHub 可达时优先在 Docker 内直接 `git -c http.version=HTTP/1.1 fetch`。确需离线传 bundle 时，不要用 PowerShell 原始二进制管道；必须在容器内校验文件大小、SHA256 和 `git bundle verify` 成功后，才能 `git fetch /tmp/<bundle>`。
- 验证：损坏 bundle 被拒绝后，改用 Docker 内直接 `git -c http.version=HTTP/1.1 fetch origin codex/ssh2-rrbs-production-optimization`，成功同步到 `9709b55`。

### 39. Docker 非登录 shell 可能没有 Cargo PATH

- 现象：SSH 进入宿主机后执行 `docker exec -i vscode-ssh2 bash`，在容器内同一 checkout 运行 `cargo build --release -p bsmap` 报 `cargo: command not found`。
- 原因：非登录、非交互 Bash 不一定加载 `$HOME/.cargo/env`，即使容器内实际安装了 Rust/Cargo。
- 规避：Docker 内构建脚本开头显式执行 `source "$HOME/.cargo/env" 2>/dev/null || true`，并设置 `export PATH="$HOME/.cargo/bin:$PATH"`；不能把 `cargo: command not found` 误判为项目编译失败。
- 验证：在 `9709b55` checkout 中加载 Cargo PATH 后，`cargo build --release -p bsmap` 通过，release binary SHA256 为 `cd988290b088fc7e905c620d1a544aba68ddb9b156db366ff906b779111620f2`。

### 40. RRBS seed mask 只影响 CountSeeds 排序，不跳过扩展扫描

- 现象：10K RRBS SE 已完全一致，但 100K 出现 2 条真实 SAM 差异；其中一条 read 在 C++ `-r 2` 下有 100 条 NM0 命中，Rust 旧实现只有 1 条。
- 原因：Rust 没有保留 C++ `xseedreg_array`/`CountSeeds()` 的 seed mask 权重语义，把含 `N` 的 seed 当成普通 seed 排序；同时扩展阶段错误使用 `reg_mask == 0` 跳过扫描。C++ 只在 CountSeeds 中用 `<< 12` 惩罚含 `N` seed 的 candidate count，`SnpAlign()` 仍扫描这些 seed。
- 规避：RRBS seed 调度必须从 `EncodedRead` mask 提取 C++ 等价 seed mask，并只用于候选计数排序；扩展阶段不得因为该 mask 跳过 seed。任何相关改动必须同时跑目标 read `-r 2`、10K 和 100K 的 streaming/sorted SAM 对比。
- 验证：提交 `9709b55` 后，两条目标 read 的默认输出和 `-r 2` sorted multiset 均与 C++ 完全一致；SSH2 100K RRBS SE 达到 streaming diff 0、sorted multiset diff 0。

### 41. `git fetch origin <branch>` 不一定更新 Docker 内 remote-tracking ref

- 现象：Docker checkout 中执行 `git fetch origin codex/ssh2-rrbs-production-optimization` 后，输出只显示 `<branch> -> FETCH_HEAD`；随后 `git reset --hard origin/codex/ssh2-rrbs-production-optimization` 把仓库退回旧提交 `da926d8`。
- 原因：该 fetch 形式只保证更新 `FETCH_HEAD`，不一定更新已有的 `refs/remotes/origin/<branch>`；reset remote-tracking ref 时可能使用陈旧引用。
- 规避：服务器 Docker 同步分支时使用显式 refspec：`git fetch origin <branch>:refs/remotes/origin/<branch>`，然后 reset 到 `refs/remotes/origin/<branch>`；或者直接 `git reset --hard FETCH_HEAD`，但必须先核对 `git rev-parse FETCH_HEAD`。
- 验证：改用显式 refspec 后，Docker checkout 从 `da926d8` 正确更新到 `ebd6f50`，`git status --porcelain` 为空。

### 42. SSH 到 Docker 的嵌套命令边界

- 现象：直接把带分号、重定向或管道的长命令传给 `ssh "... docker exec ... bash -lc '...'"` 时，外层 host shell 可能提前解释部分片段，导致写入或重定向发生在宿主机而不是 Docker 容器内。
- 原因：PowerShell、Windows `cmd`、SSH 远端 shell、`docker exec` 和容器内 Bash 之间存在多层引号与重定向边界；某一层引号断裂后，后续 `>`、`;`、`|` 会落到错误执行环境。
- 规避：服务器写入/删除使用本地 LF 临时脚本，通过 `cmd /c type <script> | ssh ... "docker exec -i vscode-ssh2 bash -s"` 输入容器 Bash；脚本内容只在 Docker 内执行。二进制或 bundle 传输使用 base64 写入容器内临时文件。
- 验证：SSH2 使用该方式将本地 Git bundle 同步到 Docker，`git bundle verify`、`git reset --hard`、`git status --porcelain` 和 `git rev-parse --short HEAD` 均在容器内完成，未再出现宿主机侧误写。

### 43. Docker reset 后必须强制重建 release binary

- 现象：Docker checkout 已 reset 到新 commit，但 benchmark metadata 中的 Rust binary SHA256 仍是旧实验二进制；10K/100K 结果复现了已回退的 query-shift 错误。
- 原因：`git reset --hard` 只更新源码，不保证 Cargo 按预期重编 release binary；旧 `target/release/bsmap` 可能因时间戳或构建缓存被复用。
- 规避：每次服务器同步新 commit 后，benchmark 前执行 `cargo clean -p bsmap` 或至少删除 `bsmap-rs/target/release/bsmap`，再 `cargo build --release -p bsmap`，并记录 binary SHA256。不得只凭 `git rev-parse HEAD` 认定二进制对应当前源码。
- 验证：SSH2 中旧 SHA `a30f8971...` 导致 10K mapped 2,746；强制重建后 SHA 变为 `18737ff...`，10K/100K 重新达到 Rust/C++ mapped 2,423/24,236 且 sorted diff 0。

### 44. C++ RRBS 末端 ZP/ZL 边界标签

- 现象：mm10 RRBS 1M SE 中 Rust/C++ sorted multiset 只剩 3 条差异，QNAME/RNAME/POS/FLAG/NM 一致，但 C++ 在 `chr4_GL456350_random:227672` 输出 `ZP=227672,ZL=139496`，Rust 不输出 ZP/ZL。
- 原因：C++ `CCGG_seglen()` 的循环先读取 `sites[right]` 再检查 `right < size`；当命中位于最后一个 CCGG site 之后的 terminal fragment 时，会表现出越界式标签。Rust 保持边界安全，不伪造该标签。
- 规避：SAM 等价报告中把这 3 条作为已知 C++ 末端标签边界差异单独列出；不要为了 100% 标签一致引入越界读取或硬编码异常。核心比对字段仍按 QNAME/RNAME/POS/FLAG/NM 判断是否一致。
- 验证：SSH2 1M 默认策略 run `/workspace/benchmark_results/ssh2/20260627T190527Z-4476/summary.json` 中，sorted multiset exact 为 253,099/253,102，expected-only 与 actual-only 样本均为这 3 条记录，差异仅 ZP/ZL。

### 45. PowerShell 管道会破坏 Git patch 文本边界

- 现象：本地用 PowerShell 管道把 `git format-patch` 输出传到 SSH/Docker 后，容器内 `git am` 报类似 `Applying: ﻿From ...`、`fatal: empty ident name`，失败后还可能残留 `.git/rebase-apply`。
- 原因：PowerShell/native pipe 会按文本对象语义重编码，可能插入 BOM、改写换行或拆分 mbox 头；`git am` 需要 byte-for-byte 的邮件补丁格式，不能容忍这类转换。
- 规避：需要跨 SSH 同步 patch/bundle 时，先在本地用 `cmd /c git ... > <file>` 生成真实文件，再对文件做 base64 文本传输；容器内校验大小、SHA256、`git am --show-current-patch` 或 `git bundle verify` 后再应用。`git am` 失败后，只有确认没有 `git am/rebase` 进程且仓库路径正确时，才在 Docker 内精确清理该仓库的 `.git/rebase-apply`。
- 验证：SSH2 中改用文件加 base64 方式后，Docker checkout 成功应用 `fe3f84a` 和 `d406835` 对应补丁，`git status --porcelain` 为空并能构建 release binary。

### 46. 源码搜索必须排除 benchmark 大结果目录

- 现象：在仓库根或 worktree 内直接 `rg` / `grep -R` 搜索 `RunAlign`、`BSMAP_PROFILE_RRBS` 等关键字，会扫到 `benchmark/results*`、历史 SAM、diff 和 comparison 文件，输出数万行甚至拖慢服务器 I/O。
- 原因：仓库里保留了大量历史 benchmark 结果和 SAM/diff 文本；这些文件内容可能包含源码关键字、read 名称或完整 SAM 记录，普通递归搜索不会自动区分源码与结果。
- 规避：源码搜索固定限制目录或排除大结果目录，例如 `rg <pattern> bsmap-rs/bsmap/src bsmap-original/bsmap-2.90 -g '!**/benchmark/results*/**' -g '!**/*.sam' -g '!**/*.diff'`。服务器结果目录只搜索 `metadata.tsv`、`summary.json`、`stderr.txt` 等小文件，禁止对整个 run 目录做宽 `grep -R`。
- 验证：SSH2 中误开的 `grep -R BSMAP_PROFILE_RRBS /workspace/benchmark_results/ssh2` 被终止后，改用 `find ... -path '*/stderr.txt' | xargs grep` 只读取小日志文件，未再影响正在运行的 full benchmark。

### 47. RRBS profiling 计数不能当作性能结果

- 现象：`BSMAP_PROFILE_RRBS=1` 的 1M RRBS SE run wall 从默认生产路径约 35.77 秒膨胀到 197.48 秒，但 mapped 和 sorted diff 口径不变。
- 原因：counts profile 在候选热路径上为 `segment_calls`、`mode_matched_candidates`、`mismatch_calls` 等计数执行大量 atomic fetch-add；该开销会严重污染 wall/user time。
- 规避：`BSMAP_PROFILE_RRBS=1` 只用于判断候选规模和热点方向，不用于 Rust/C++ 性能对比。正式性能表必须使用 `SSH2_PROFILE_RRBS=0` 或未开启 profile 的 binary/run；若需要阶段耗时，优先使用低频 stage 日志或外部采样。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260627T223922Z-10085/summary.json` 中，1M counts profile 记录 `mismatch_calls=1967547050`，但 Rust wall 为 197.48 秒；同 commit 默认 run `/workspace/benchmark_results/ssh2/20260627T223149Z-9798/summary.json` 的 1M Rust wall 为 35.77 秒。

### 48. RRBS hit 顺序优化必须检查 streaming SAM

- 现象：尝试把 RRBS v11 hit layout 改为每个 mode 先 normal 再 BSC 后，1M Rust wall 仅从 35.77 秒降到 35.21 秒；100K sorted multiset diff 为 0，但 streaming compare 从 record 1 起出现大面积 QNAME/RNAME/POS 顺序差异。
- 原因：RRBS hit 存储顺序参与 C++ 风格随机 bucket 起点、环形遍历和输出顺序；即使最终记录集合相同，改变 hit 顺序也可能破坏与 C++ 的 streaming SAM 对齐。
- 规避：任何 RRBS index hit 重排、normal-only slice、compact bucket 或随机遍历优化，必须同时检查 streaming diff 和 sorted multiset diff。若收益小且 streaming 顺序变动，应撤回；不得只用 sorted diff 0 判定可保留。
- 验证：SSH2 v11 run `/workspace/benchmark_results/ssh2/20260627T234727Z-12600/summary.json` 显示 100K sorted exact 24,236/24,236 但 streaming exact 0/24,236；候选提交 `ee524cf` 已由 `4804347` revert。

### 49. AddHit 去重 hash 不是 SSH2 当前主瓶颈

- 现象：把 `HashSet<(u32,u32)>` 改成 packed `u64` key 加 identity hasher 后，10K/100K SAM diff 仍为 0，1M 仍只剩已知 3 条 C++ terminal ZP/ZL 差异，但 1M Rust wall 从保留基线 35.77 秒退到 36.44 秒，RSS 基本不变。
- 原因：SSH2 当前主要成本仍来自 RRBS 候选规模和 mismatch kernel；`AddHit` 去重 hash 成本不是端到端主导项。替换 hasher 还会增加实现复杂度，收益不成立。
- 规避：不要继续围绕 `AddHit` 去重 hash 做小改，除非新的 profile 证明 accepted-hit 去重占比已经成为主瓶颈。去重结构变化必须同时跑 10K/100K/1M SAM diff 和性能表，不能只凭理论上 identity hash 更快就保留。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260628T002013Z-13830/summary.json` 中，candidate commit `dcb61c2` 的 1M Rust wall 为 36.44 秒；已由 `e64aaca` revert。

### 50. RRBS normal hit 解码缓存收益不足且吃 RSS

- 现象：运行时按 `(seed_hash, mode)` 解码并缓存 RRBS normal hits 后，1M Rust wall 从保留基线 35.77 秒降到 34.99 秒，但 RSS 从 1,856,048 KiB 增到 2,076,696 KiB；100K/1M streaming compare 仍出现大面积顺序差异，只有 sorted multiset 保持既有语义水平。
- 原因：SSH2 当前热路径主要仍是候选规模和 mismatch 调用，避免 packed hit 解码/BSC skip 只能带来约 2.2% 小收益；缓存本身需要额外保存 normal hits 和 mode ranges，会显著增加驻留内存。
- 规避：不要默认引入 RRBS decoded normal-hit 全量缓存。除非后续 profile 证明 packed decode 已成为主瓶颈，并且端到端收益超过门槛、RSS 仍低于目标，否则优先优化候选规模、mismatch kernel 或 pipeline。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260628T004435Z-14681/summary.json` 中，candidate commit `f143c44` 的 1M Rust wall 为 34.99 秒、RSS 为 2,076,696 KiB；已由 `a82f138` revert。

### 51. per-read N count 缓存不是 SSH2 当前主瓶颈

- 现象：把 `count_n_in_mask(mask, read_len)` 从每个 segment 重复计算改成每条 read/read-chain 预计算一次后，1M Rust wall 只从保留基线 35.77 秒降到 35.59 秒，RSS 基本持平。
- 原因：虽然 1M 有约 971 万次 segment 调用，但 N 计数相对 19.7 亿次 mismatch 调用不是端到端主导成本；该优化只能带来约 0.5% 的短基准收益。
- 规避：不要继续围绕 per-read 固定小字段预计算做零散微调，除非 profile 证明相关函数进入 top hotspot。SSH2 后续应优先优化候选规模、mismatch kernel、或能显著降低 full align core 的数据流结构。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260628T010232Z-15433/summary.json` 中，candidate commit `5b1e4aa` 的 1M Rust wall 为 35.59 秒、RSS 为 1,857,736 KiB；已由 `95798d8` revert。

### 52. Docker 内 `perf` 存在不等于可采样

- 现象：Docker 内 `/usr/bin/perf` 和 `/usr/bin/timeout` 存在，`/proc/sys/kernel/perf_event_paranoid` 为 `2`，但执行 `perf stat` 时输出 `WARNING: perf not found for kernel 3.10.0`，并提示需要安装对应内核的 `linux-tools-3.10.0`。
- 原因：容器里的 `perf` 用户态工具与宿主机内核版本不匹配，且容器不能自行提供正确的内核 perf 事件支持；工具文件存在不能证明采样链路可用。
- 规避：SSH2 阶段不要继续依赖 Docker 内 `perf stat/perf record` 判断 RRBS 热点。若要采样，改用低开销 stage 日志、短样本诊断计数、源码审计，或在宿主机上另建经验证的 perf/eBPF 环境；正式 Rust/C++ 性能表不得混入 perf 失败 run。
- 验证：SSH2 probe 结果目录 `/workspace/benchmark_results/ssh2/perf-probe-20260628T011156Z-15865` 记录了该 stderr；后续优化判断改回生产 binary 的 10K/100K/1M/full benchmark。

### 53. mismatch reference-window 分支拆分没有端到端收益

- 现象：把 `count_mismatch()` 的常规 `bit_offset != 0` 路径拆成无末端越界分支的 slice 访问后，本地编译测试通过，10K/100K/1M mapped 和分布不变，但 1M Rust wall 从保留基线 35.77 秒退到 35.90 秒，RSS 从 1,856,048 KiB 略增到 1,857,704 KiB。
- 原因：当前瓶颈虽在 mismatch 调用规模，但这个局部分支不是端到端主导项；额外分支拆分和 slice 形态没有压过编译器原有优化，短样本小幅波动不能外推为 full 收益。
- 规避：不要继续围绕 `count_mismatch()` 内部边界分支做无证据微调。后续 mismatch 优化必须先证明能减少大量 candidate、减少 word 计算，或引入经端到端验证的专用 kernel；仅重排安全分支/iterator 形态不足以保留。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260628T011859Z-16179/summary.json` 中，candidate commit `01a5564` 的 1M Rust wall 为 35.90 秒、RSS 为 1,857,704 KiB；已由 `622c7d9` revert。

### 54. 只提取启用 read-chain 的 seed 不是 SSH2 当前主瓶颈

- 现象：SE 默认 `-n 0` 下只扩展 read_chain 0，因此尝试跳过 read_chain 1 的 seed/mask 提取；本地编译测试通过，10K/100K/1M mapped 和 sorted SAM 语义保持，但 1M Rust wall 从保留基线 35.77 秒退到 36.21 秒，RSS 从 1,856,048 KiB 略增到 1,857,664 KiB。
- 原因：full RRBS SE 当前主要成本来自候选规模和 mismatch 调用，禁用链 seed 提取只是每条 read 的小固定开销；减少这部分工作没有转化为端到端收益。
- 规避：不要继续围绕 SE 禁用链的 seed 提取、固定 scratch 清空或类似 per-read 小固定项做 SSH2 主线优化。除非 profile 证明 prepare/seed extraction 已成为主热点，否则应优先减少 mode-matched candidates、mismatch calls 或改变能显著压缩 align core 的数据流。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260628T013109Z-16808/summary.json` 中，candidate commit `44a7e55` 的 1M Rust wall 为 36.21 秒、RSS 为 1,857,664 KiB；已由 `3272fc5` revert。

### 55. `xm64()` 改成 `count_ones()` 在当前构建下更慢

- 现象：将 `xm64()` 从 C++ 风格 SWAR byte-sum 改成 `((tt | tt >> 1) & 0x5555...).count_ones()` 后，本地编译测试通过，10K/100K/1M mapped 和 sorted SAM 语义保持，但 1M Rust wall 从保留基线 35.77 秒退到 36.79 秒。
- 原因：当前默认 release 构建未证明 `count_ones()` 会生成更快的硬件 popcount 路径；即便语义等价，替换手写 SWAR 会在 mismatch 热路径造成端到端退化。
- 规避：不要在默认生产构建里把 `xm64()` 改成 `count_ones()`。若以后重新评估 popcount/SIMD，必须以明确 target feature、microbench 和 10K/100K/1M 端到端共同证明收益，不能只凭“硬件 popcount 理论上更快”保留。
- 验证：SSH2 run `/workspace/benchmark_results/ssh2/20260628T014304Z-17440/summary.json` 中，candidate commit `9a33a7b` 的 1M Rust wall 为 36.79 秒、RSS 为 1,857,684 KiB；已由 `cce4f2b` revert。

### 56. `target-cpu=native` 是显式本机构建小收益，不是 portable 默认值

- 现象：同一 SSH2 commit `3a18390` 在 Docker 内使用 `RUSTFLAGS="-C target-cpu=native"` 构建后，1M RRBS SE wall 从 portable 保留基线 35.77 秒降到 34.18 秒，RSS 基本持平；10K/100K SAM diff 仍为 0，1M 仍只剩既有 3 条 C++ terminal ZP/ZL 差异。
- 原因：CPU-specific codegen 对当前服务器热路径有小幅收益，但它不减少 1M 约 19.2 亿次 mismatch 调用，也不能把 full SE 从 926 秒级压到 `C++ full / 2` 所需的 525 秒以内。
- 规避：`target-cpu=native` 只能作为明确标记的本机/部署机器构建方式使用，报告必须记录 RUSTFLAGS、CPU 环境和 binary SHA256；不得静默替代 portable release，也不得把 native 小收益写成 full 目标已完成。验证后若继续跑 portable 基准，必须恢复标准 release binary。
- 验证：native run `/workspace/benchmark_results/ssh2/20260628T015428Z-20368/summary.json` 中，native binary SHA256 为 `c80f886d5703b93eadf50e1229cd66b926d1660a4572f7034e58a9f66545e0e6`，1M Rust wall/RSS 为 34.18 秒/1,856,024 KiB；随后已恢复 portable binary SHA256 `48199b5d47ba278e9fa9885798bd083e70e1235c4e6b9ab6578a2c0f6a331afb`。

### 57. SSH2 当前 RRBS SE 候选规模已经与 C++ 等价

- 现象：100K RRBS SE 诊断中，Rust logical candidates 为 590,799,207，C++ raw candidates 也是 590,799,207；Rust/C++ mismatch calls 均为 191,939,381，accepted hits 均为 1,357,106，SAM streaming/sorted diff 均为 0。
- 原因：P13 之后的 mode/read-chain/BSC logical bucket 语义已经对齐 C++；Rust profile 中更大的 raw candidates 包含 BSC/cross-chain 物理项，但 SE logical bucket 会按 C++ 语义排除，实际进入 mismatch 的候选数没有额外放大。
- 规避：不要继续凭直觉通过丢弃 RRBS candidates 来追求速度，除非有新的 C++ 源码证据和 profile 证明 Rust 确实多扫。SSH2 后续大收益方向应集中在每候选 mismatch kernel、批处理、内存访问和并行流水线，而不是再改 mode/BSC 过滤语义。
- 验证：Docker 临时 C++ 插桩 run `/workspace/benchmark_results/ssh2/20260628T020344Z-21229/summary.json` 使用参数 `-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1 -E 100000`；临时 C++ profile binary SHA256 为 `75910344186240f7a6fc9275f9cf04ac3d9eaf9d6e1741eec310dd16cc653491`。

### 58. RRBS mismatch query-shift 不能直接替代现有 reference-shift

- 现象：尝试把 RRBS `count_mismatch()` 改成 C++ 风格 query/mask shift 后，新旧算法等价单测在 `read_len=33`、`reference_start=2`、`threshold=100` 下失败：query-shift 返回 25，现有 reference-shift oracle 返回 24。
- 原因：当前 Rust 的 `xc64` 容忍掩码、word 边界和 padding 语义不是简单的逐 word query/mask shift；直接移动 query/mask 会在跨 word 边界时改变 mismatch 计数。此前 Rust reference-shift 路径已经通过大样本 SAM 等价验证，不能为了局部指令形态牺牲 correctness gate。
- 规避：不要直接把 C++ `CountMismatch()` 指针公式照搬成 Rust query-shift fast path。若未来继续做专用 mismatch kernel，必须先用新旧 Rust oracle 覆盖 read length、offset、mask/N、threshold，再跑 10K/100K/1M SAM diff；未通过单测不得进入 Docker benchmark。
- 验证：WSL2 定向测试 `cargo test -p bsmap test_count_mismatch_query_shift_matches_reference_shift` 在上述 case 失败；该候选已在本地立即撤回，未提交、未跑服务器性能测试。

### 59. RRBS SE pipeline depth 继续加深收益不足

- 现象：在默认保留的 RRBS SE depth=2 pipeline 之后，显式测试 `--pipeline-depth 4` 和 `--pipeline-depth 8`；1M depth=4 wall 为 35.92 秒，depth=8 wall 为 35.30 秒，均保持 sorted SAM 既有差异水平，但 depth=8 RSS 增至 1,887,492 KiB。
- 原因：当前 1M/full RRBS SE 的 CPU 利用率已经接近 8 线程上限，继续加深 producer/align 缓冲无法减少每候选 mismatch 工作量；更深 pipeline 主要增加缓冲驻留，收益只剩噪声级或小幅 I/O 重叠。
- 规避：不要继续把 SSH2 主线放在调大 `--pipeline-depth` 上。默认 depth=2 仍是当前保留点；除非新的大样本 stage timing 证明读取/写出重新成为瓶颈，否则后续应转向 mismatch kernel、批处理或更深的数据结构优化。
- 验证：Docker probe `/workspace/benchmark_results/ssh2/pipeline-depth-probe-20260628T021507Z-21614` 使用 portable binary SHA256 `48199b5d47ba278e9fa9885798bd083e70e1235c4e6b9ab6578a2c0f6a331afb`；depth=4/8 均为 253,102 mapped，sorted exact 253,099/253,102。

### 60. RRBS SE pipeline stage profile 显示读写不是 SSH2 主瓶颈

- 现象：为默认 depth=2 pipeline 补充低开销 stage timing 后，1M RRBS SE wall 为 36.07 秒，其中 read 2.55 秒、prepare 1.18 秒、align 25.46 秒、write 1.51 秒；SAM sorted diff 仍只是既有 3 条 C++ terminal ZP/ZL 差异。
- 原因：pipeline 已经重叠了大部分读写与比对工作，端到端剩余成本主要集中在每候选 mismatch/extend 核心；继续优化 FASTQ 读取、SAM 写出或 pipeline 深度的理论上限太小。
- 规避：SSH2 后续不要再把主线放在浅层 I/O、写出格式化或 pipeline-depth 调参上，除非新的 full stage profile 证明占比变化。大收益候选应优先来自 mismatch kernel、批量化候选处理、reference/index 访问局部性或更大粒度并行调度。
- 验证：提交 `b93cfe3` 后 Docker run `/workspace/benchmark_results/ssh2/20260628T022458Z-21945/summary.json` 使用 portable binary SHA256 `2c0afe7204b5ca423935cdcbef03cf5e2830e4fdc517f257a3bd2a204b7f5488`；1M Rust/C++ 均为 253,102 mapped，Rust RSS 1,855,968 KiB，C++ RSS 2,486,468 KiB。

### 61. Windows PowerShell 不能假定支持 `&&`

- 现象：在本地 worktree 执行 `git add ... && git diff --cached --stat && git commit ...` 时，PowerShell 报 `The token '&&' is not a valid statement separator in this version`，Git 命令没有执行。
- 原因：当前 Windows PowerShell 环境不是支持 `&&` pipeline chain operator 的新版 PowerShell；直接使用 Bash 风格命令串会在解析阶段失败。
- 规避：本地关键 Git 和验证命令在 PowerShell 中分步执行，或显式使用 WSL/Bash 并开启 `set -euo pipefail`。不要把未执行的 chained command 误判为 Git 失败或代码失败。
- 验证：改为分步执行 `git add`、`git diff --cached --stat`、`git diff --cached --check` 和 `git commit` 后，`b93cfe3 profile: add RRBS pipeline stage timing` 成功提交并推送。

### 62. `count_mismatch()` 强制 inline 收益不足

- 现象：将 `count_mismatch()` 从普通 `#[inline]` 改为 `#[inline(always)]` 后，本地 `cargo check/test/build` 通过，10K/100K SAM diff 为 0，1M sorted diff 仍只是既有 3 条 C++ terminal ZP/ZL；但 1M Rust wall 只从保留基线 35.77 秒降到 35.18 秒。
- 原因：函数边界不是当前 SSH2 full RRBS SE 的主瓶颈；该变化约 1.6% 的短基准收益低于保留门槛，也可能属于 run-to-run 波动。强制 inline 还会降低编译器自主权，未来可能增加代码体积或造成不同 CPU/编译器下的回归。
- 规避：不要把 `#[inline(always)]` 作为 SSH2 mismatch 主线优化手段。后续 mismatch 优化必须证明能显著减少每候选 word 工作、引入正确的批量/SIMD kernel，或改善 reference/index 访问局部性，并用 10K/100K/1M SAM diff 与性能表共同验证。
- 验证：候选 `87f0c49` 的 Docker run `/workspace/benchmark_results/ssh2/20260628T023417Z-22324/summary.json` 中，1M Rust wall/RSS 为 35.18 秒/1,856,020 KiB；已由 `2cc6c37 Revert "perf: force inline mismatch counter"` 撤回。

### 63. full RRBS SE stage profile 证明主瓶颈仍是 align core

- 现象：full RRBS SE 使用 `BSMAP_PROFILE_RRBS=stage` 后，Rust warm align wall 为 933.61 秒，其中 read 86.18 秒、prepare 41.19 秒、align 863.54 秒、write 51.50 秒；RSS 1,858,428 KiB，mapped 8,873,078。
- 原因：full 数据上 pipeline 已经把读、prepare 和写出的大部分开销与比对重叠，剩余绝对主耗时来自 RRBS align/mismatch/extend 核心。即使理想化清零读、prepare 和 SAM write，也无法达到 SSH2 的 `C++ full / 2` 目标。
- 规避：除非新的 full profile 反证，不要继续把 SSH2 主线放在 FASTQ 解压、SAM writer、pipeline depth 或浅层调参上。后续高收益方向应集中在每候选 mismatch 成本、批量化候选处理、reference/index 随机访问局部性和更大粒度并行调度。
- 验证：Docker run `/workspace/benchmark_results/ssh2/rust-full-stage-20260628T024823Z-22811/summary.json` 使用 commit `64c9c66`、portable binary SHA256 `2c0afe7204b5ca423935cdcbef03cf5e2830e4fdc517f257a3bd2a204b7f5488`；run 前后 v10 index SHA256 均为 `1329966ddda5aedd9fc7e13cb84a4e755cd632df3d14a0de32a239a29561e634`。
