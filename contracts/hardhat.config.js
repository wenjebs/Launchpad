export default {
  networks: {
    hardhat: {
      type: "edr-simulated",
      allowBlocksWithSameTimestamp: true,
      mining: {
        auto: true,
        interval: 0,
      },
    },
  },
};
