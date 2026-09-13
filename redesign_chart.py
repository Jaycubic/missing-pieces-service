"""
DESG 317 - Assignment 2, Part 2: Redesign (Full-size version)
Diverging benchmark ("lollipop") plot of PP100 vs. industry average.
Design authored by student; this script only implements the specified layout.
"""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker

# ---- Data (verified against J.D. Power 2024 U.S. Vehicle Dependability Study,
#      as published by Visual Capitalist) ----
data = [
    ("Lexus", 135), ("Toyota", 147), ("Buick", 149), ("Chevrolet", 174),
    ("MINI", 174), ("Porsche", 175), ("Mazda", 185), ("Kia", 187),
    ("BMW", 190), ("Dodge", 190), ("Jeep", 190), ("Cadillac", 196),
    ("Hyundai", 198), ("Subaru", 198), ("Nissan", 199), ("Genesis", 200),
    ("Ram", 201), ("GMC", 206), ("Honda", 206), ("Acura", 216),
    ("Mercedes-Benz", 218), ("Infiniti", 219), ("Ford", 239), ("Volvo", 245),
    ("Lincoln", 251), ("Volkswagen", 267), ("Land Rover", 268), ("Audi", 275),
    ("Chrysler", 310),
]
BASELINE = 190

# sort best (lowest PP100) to worst (highest PP100)
data_sorted = sorted(data, key=lambda x: x[1])
brands = [d[0] for d in data_sorted]
values = [d[1] for d in data_sorted]
diffs = [v - BASELINE for v in values]
n = len(brands)

GREEN = "#1a7a3c"
RED = "#c81d3f"
GRAY = "#8a8a8a"

def color_for(d):
    if d < 0:
        return GREEN
    elif d > 0:
        return RED
    return GRAY

colors = [color_for(d) for d in diffs]

fig, ax = plt.subplots(figsize=(11, 15))
fig.patch.set_facecolor("#f7f6f2")
ax.set_facecolor("#f7f6f2")

y_pos = range(n)

# stems
for y, d, c in zip(y_pos, diffs, colors):
    ax.hlines(y, 0, d, color=c, linewidth=2.2, zorder=3)
# markers
ax.scatter(diffs, list(y_pos), color=colors, s=70, zorder=4, edgecolor="white", linewidth=0.8)

ax.invert_yaxis()
ax.set_yticks(list(y_pos))
ax.set_yticklabels(brands, fontsize=11, fontweight="bold")

# data labels: "{+/-diff} ({value})"
for y, d, v, c in zip(y_pos, diffs, values, colors):
    label = f"{d} ({v})" if d == 0 else f"{d:+d} ({v})"
    if d >= 0:
        ax.text(d + 3, y, label, va="center", ha="left", fontsize=9.5, color=c, fontweight="bold")
    else:
        ax.text(d - 3, y, label, va="center", ha="right", fontsize=9.5, color=c, fontweight="bold")

# baseline reference line
ax.axvline(0, color="#222222", linestyle="--", linewidth=1.3, zorder=2)
ax.text(0, -1.9, f"Industry Average\n({BASELINE} PP100)", ha="center", va="bottom",
        fontsize=9.5, color="#222222", fontweight="bold")

# directional headers
ax.text(-70, -3.1, "\u2190 FEWER PROBLEMS\n(Higher Dependability)", ha="left", va="bottom",
        fontsize=10.5, color=GREEN, fontweight="bold")
ax.text(70, -3.1, "MORE PROBLEMS \u2192\n(Lower Dependability)", ha="right", va="bottom",
        fontsize=10.5, color=RED, fontweight="bold")

# gridlines & axis
ax.set_xlim(-80, 130)
ax.xaxis.set_major_locator(mticker.MultipleLocator(25))
ax.grid(axis="x", color="#d9d7d0", linewidth=0.7, linestyle=":", zorder=0)
ax.set_axisbelow(True)
for spine in ["top", "right", "left"]:
    ax.spines[spine].set_visible(False)
ax.spines["bottom"].set_color("#999999")

ax.set_xlabel("Difference in Problems per 100 Vehicles (PP100) vs. 190 Baseline",
              fontsize=10.5, color="#333333", fontweight="bold", labelpad=12)

# title & subtitle
fig.text(0.09, 0.975, "Vehicle Dependability: Deviation from Industry Average",
          fontsize=19, fontweight="bold", color="#1a1a1a")
fig.text(0.09, 0.958, "Problems per 100 vehicles (PP100) relative to 190 PP100 baseline | J.D. Power 2024 Study",
          fontsize=11, color="#444444")

# footer / source
fig.text(0.09, 0.012,
          "Source: J.D. Power 2024 U.S. Vehicle Dependability Study (2021 model-year vehicles, surveyed after 3 years of ownership).",
          fontsize=8, color="#777777")
fig.text(0.09, 0.002, "Redesign, DESG 317 Assignment 2 \u2014 Diverging Benchmark Plot", fontsize=8, color="#999999")

plt.tight_layout(rect=[0, 0.03, 1, 0.945])
output_path = "redesign_fullsize.png"
plt.savefig(output_path, dpi=200, facecolor=fig.get_facecolor())
print(f"done: saved to {output_path}")

