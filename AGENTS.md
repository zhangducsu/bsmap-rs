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

- 现象：从 PowerShell 调用 WSL 时写 `env THREAD_MATRIX="1 2 4 8 16" bash ...`，脚本没有执行线程矩阵，反而打印了环境变量列表。
- 原因：PowerShell、`wsl.exe` 和 Bash 的多层参数边界会重新拆分带空格的环境变量值；外层看似已经加引号，进入 Linux 侧后仍可能被拆成多个 argv。
- 规避：正式 benchmark 不通过一行命令传递带空格的 env 值；优先使用脚本默认矩阵，或在 Bash 脚本内部/临时配置文件中设置。需要传递时用完整单引号 Bash command 并在 Linux 侧 `printf '%q\n' "$THREAD_MATRIX"` 自检。
- 验证：去掉跨边界的 `THREAD_MATRIX="1 2 4 8 16"` 后，`run_thread_matrix.sh` 默认生成 p1/p2/p4/p8/p16 共 15 个 run，并输出 `thread_matrix.json`。

### 29. DrvFS 会放大 mm10 RRBS 索引与比对的缺页和 wall time

- 现象：同一 v10 mm10 RRBS 索引在 D 盘 DrvFS 构建约 64.00 秒，在 WSL ext4 forward-only 构建约 33.86 秒；RRBS alignment 在 ext4 上 major faults 接近 50 到 60，而 DrvFS v9 约 20 万级。
- 原因：DrvFS 通过 Windows 文件系统桥接大 mmap/random access，页缓存、缺页和 metadata 行为与原生 Linux ext4 差异很大；这会掩盖 Rust 索引布局本身的真实收益。
- 规避：正式 Linux/部署性能数字使用 WSL ext4 或服务器 Docker ext4/overlay；D 盘 DrvFS 结果只能作为 Windows 文件系统限制记录。大型输入和结果可留在 D 盘，但会被频繁 mmap 的 `.bsi` 和 reference 应复制/硬链接到 ext4。
- 验证：v10 forward-only ext4 index SHA 与 DrvFS v10 index SHA 均为 `d7afbc84...`，说明数据等价；性能差异来自文件系统路径而非索引内容。
