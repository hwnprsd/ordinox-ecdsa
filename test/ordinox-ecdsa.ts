import {
  time,
  loadFixture,
} from "@nomicfoundation/hardhat-toolbox/network-helpers";
import { anyValue } from "@nomicfoundation/hardhat-chai-matchers/withArgs";
import { expect } from "chai";
import { ethers } from "hardhat";

describe("", function () {
  async function deployOneYearLockFixture() {
    const [owner] = await ethers.getSigners();

    const Router = await ethers.getContractFactory("RdnxRouter");
    const router = await Router.deploy();

    return { router, owner };
  }

  describe("Deployment", function () {
    it("Should test the router", async function () {
      const { router } = await loadFixture(deployOneYearLockFixture);
      const res = await router.transferOutWithSignature(
        "0x729D92187A787B90237Da8dAb2Dc2baA12ca2f4d",
        "0x000000000000000000000000000000000000beef",
        "0x000000000000000000000000000000000000dead",
        1,
        1,
        1,
        "0x2995502eab24b3093fc496c5b2740879fafe96be94b592ec9817568daab1f94d4399f6496118f845799b5f782dc4a66a9cc4656b02c249e12846b7b144a183241c"
      );
      console.log({ res });
    });
  });
});
