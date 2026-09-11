/** Build-time config only. See package.json's "build" script. */
module.exports = {
  content: ["../public/index.html"],
  theme: {
    extend: {
      colors: {
        ink: "#14201F",
        surface: "#1B2B2A",
        border: "#2C4443",
        // Five EQ-pillar accents, reused across both games' documentation
        pillar: {
          awareness: "#E0A73C",
          regulation: "#3C8FE0",
          motivation: "#E0623C",
          empathy: "#3CE0A0",
          social: "#B03CE0",
        },
      },
      fontFamily: {
        display: ["Iowan Old Style", "Palatino Linotype", "Georgia", "serif"],
      },
    },
  },
  plugins: [],
};
