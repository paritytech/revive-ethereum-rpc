import { BrowserProvider, Contract, ContractFactory, JsonRpcSigner, parseEther } from 'ethers';
import { Vector, u8, Struct } from 'scale-ts';
import { hexToU8a } from '@polkadot/util';

const CallInput = Struct({
  code: Vector(u8),
  data: Vector(u8),
  salt: Vector(u8),
});

document.addEventListener('DOMContentLoaded', async () => {
  if (typeof window.ethereum == 'undefined') {
    return console.log('MetaMask is not installed');
  }

  console.log('MetaMask is installed!');
  const provider = new BrowserProvider(window.ethereum);

  console.log('Getting signer...');
  let signer: JsonRpcSigner;
  try {
    signer = await provider.getSigner();
    console.log(`Signer: ${signer.address}`);
  } catch (e) {
    console.error('Failed to get signer', e);
    return;
  }

  console.log('Getting block number...');
  try {
    const blockNumber = await provider.getBlockNumber();
    console.log(`Block number: ${blockNumber}`);
  } catch (e) {
    console.error('Failed to get block number', e);
    return;
  }

  const nonce = await signer.getNonce();
  console.log(`Nonce: ${nonce}`);

  document.getElementById('transactButton')?.addEventListener('click', async () => {
    await transact();
  });

  document.getElementById('deployButton')?.addEventListener('click', async () => {
    await deploy();
  });
  document.getElementById('deployAndCallButton')?.addEventListener('click', async () => {
    const address = await deploy();
    await call(address);
  });
  document.getElementById('callButton')?.addEventListener('click', async () => {
    await call();
  });

  async function deploy() {
    console.log('Deploying contract...');

    const bytecode = CallInput.enc({
      code: Array.from(
        hexToU8a(
          '0x0061736d01000000010401600000020f0103656e76066d656d6f727902000103020100040501700101010616037f01418080040b7f00418080040b7f00418080040b0711020463616c6c0000066465706c6f7900000a040102000b0022046e616d65010701000463616c6c071201000f5f5f737461636b5f706f696e746572004d0970726f64756365727302086c616e6775616765010452757374000c70726f6365737365642d6279010572757374631d312e37372e32202832356566396533643820323032342d30342d303929002c0f7461726765745f6665617475726573022b0f6d757461626c652d676c6f62616c732b087369676e2d657874',
        ),
      ),
      data: [],
      salt: [1],
    });

    const contractFactory = new ContractFactory([], bytecode, signer);

    try {
      const contract = await contractFactory.deploy();
      const address = await contract.getAddress();
      console.log(`Contract deployed: ${address}`);
      return address;
    } catch (e) {
      console.error('Failed to deploy contract', e);
      return;
    }
  }

  async function call(address = '0xad3036bd55714921555185a9e1c6a126802aebbc') {
    const abi = ['function getValue() view returns (uint256)', 'function setValue(uint256 _value)'];
    const contract = new Contract(address, abi, signer);
    const tx = await contract.setValue(42);

    console.log('Transaction hash:', tx.hash);
  }

  async function transact() {
    console.log('Sending transaction...');
    try {
      const tx = await signer.sendTransaction({
        to: '0x3d3593927228553b349767ABa68d4fb1514678CB',
        value: parseEther('1.0'),
      });

      const receipt = await tx.wait();
      console.log(`Transaction hash: ${receipt?.hash}`);
    } catch (e) {
      console.error('Failed to send transaction', e);
      return;
    }
  }
});
