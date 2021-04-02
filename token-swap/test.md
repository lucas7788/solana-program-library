
启动本地测试节点
```
solana-test-validator --bpf-program 3dSBFmJ6zvMmeDCXrP1N9CXBYs65rbHxpCRRyHSRgyZD ../../target/deploy/spl_token_swap.so --reset --quiet
```

运行测试代码
```
SWAP_PROGRAM_OWNER_FEE_ADDRESS="A436YAwCXmtSSP1HYnoAcBJpU8hZ8934xvNg9nVGP5gf" npm run start
```
