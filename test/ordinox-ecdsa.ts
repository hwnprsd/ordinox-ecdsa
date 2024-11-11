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
        "0x9154877f645d20c1514fccb1e10b956d537d847d65e07a651b2fea7c7b707ff1216646e12aca0f0d551ed70ed0a88d17cc114a7d21ef797ccb92f82376e419f31b"
      );
      console.log(res);
    });
  });
});
