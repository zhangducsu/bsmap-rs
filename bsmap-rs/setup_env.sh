apt-get update
apt-get install -y build-essential curl wget git python3 python3-pip time
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. /root/.cargo/env
rustup default stable
chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh
chmod +x /workspace/bsmap-rs/test_inside_container.sh
echo 'Environment ready'
