"""
DESG 317 - Assignment 2, Part 2: Redesign (Full-size + Phone versions)
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

data_sorted = sorted(data, key=lambda x: x[1])

GREEN = "#1a7a3c"
RED = "#c81d3f"
GRAY = "#8a8a8a"
BG = "#ffffff"

def color_for(d):
    if d < 0:
        return GREEN
    elif d > 0:
        return RED
    return GRAY


# =========================================================================
# 1. FULL-SIZE VERSION
# =========================================================================
def build_fullsize(path="redesign_fullsize.png"):
    brands = [d[0] for d in data_sorted]
    values = [d[1] for d in data_sorted]
    diffs = [v - BASELINE for v in values]
    n = len(brands)
    colors = [color_for(d) for d in diffs]

    fig, ax = plt.subplots(figsize=(11, 15))
    fig.patch.set_facecolor(BG)
    ax.set_facecolor(BG)

    y_pos = range(n)

    # stems
    for y, d, c in zip(y_pos, diffs, colors):
        ax.hlines(y, 0, d, color=c, linewidth=2.2, zorder=3)
    # markers
    ax.scatter(diffs, list(y_pos), color=colors, s=70, zorder=4, edgecolor="white", linewidth=0.8)

    ax.invert_yaxis()
    ax.set_yticks(list(y_pos))
    ax.set_yticklabels(brands, fontsize=11, fontweight="bold")

    # data labels: "{+/-diff} ({value})" -- original encoding, kept by design decision
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
    fig.text(0.5, 0.975, "U.S. Car Brands Dependability Compared With the Industry Average",
              fontsize=19, fontweight="bold", color="#1a1a1a", ha="center")
    fig.text(0.5, 0.958,
              "Problems per 100 vehicles (PP100) relative to the 190 U.S. industry average | J.D. Power 2024 Study",
              fontsize=11, color="#444444", ha="center")

    # footer / source -- signature line removed
    fig.text(0.5, 0.012,
              "Source: J.D. Power 2024 U.S. Vehicle Dependability Study (2021 model-year vehicles, surveyed after 3 years of ownership).",
              fontsize=8, color="#777777", ha="center")

    plt.tight_layout(rect=[0, 0.03, 1, 0.945])
    plt.savefig(path, dpi=200, facecolor=fig.get_facecolor())
    plt.close(fig)
    print(f"done: saved to {path}")


# =========================================================================
# 2. PHONE VERSION (~390px wide) -- re-aggregated, not shrunk.
#    Top 5 / Bottom 5 shown in full using the same "{diff} (value)" encoding
#    as the full-size chart; the 19 middle brands (small, less decision-
#    relevant differences) are compressed into one unlabeled distribution strip.
# =========================================================================
def build_mobile(path="redesign_mobile.png"):
    n_total = len(data_sorted)
    top5 = data_sorted[:5]
    bottom5 = data_sorted[-5:]
    middle19 = data_sorted[5:-5]

    n_better = sum(1 for _, v in data_sorted if v - BASELINE < 0)
    n_avg = sum(1 for _, v in data_sorted if v - BASELINE == 0)
    n_worse = sum(1 for _, v in data_sorted if v - BASELINE > 0)

    mid_values = [v for _, v in middle19]
    mid_lo, mid_hi = min(mid_values), max(mid_values)

    # Canvas rendered at 2x for crispness, representing a 390pt-wide phone frame
    W_PT, DPI = 3.9, 200  # 3.9in x 200dpi = 780px physical (~390pt @2x)
    fig = plt.figure(figsize=(W_PT, 10.2))
    fig.patch.set_facecolor(BG)

    # All standalone text placed via fig.text() in figure-fraction coordinates
    fig.text(0.5, 0.975,
              "U.S. Car Brands Dependability",
              fontsize=15, fontweight="bold", color="#1a1a1a", ha="center")
    fig.text(0.5, 0.958,
              f"Compared with the {BASELINE} PP100 U.S. industry average",
              fontsize=8.5, color="#444444", ha="center")

    # ---- Tally bar ----
    ax_tally = fig.add_axes([0.08, 0.918, 0.86, 0.028])
    ax_tally.axis("off")
    ax_tally.set_xlim(0, 1); ax_tally.set_ylim(0, 1)
    seg_widths = [n_better, n_avg, n_worse]
    seg_colors = [GREEN, GRAY, RED]
    x = 0
    centers = []
    for w, c in zip(seg_widths, seg_colors):
        frac = w / n_total
        ax_tally.add_patch(plt.Rectangle((x, 0), frac, 1, facecolor=c, edgecolor=BG, linewidth=1))
        centers.append(x + frac / 2)
        x += frac
    labels = [f"{n_better} better", f"{n_avg} at avg", f"{n_worse} worse"]
    for cx, c, lab in zip(centers, seg_colors, labels):
        fig.text(0.08 + cx * 0.86, 0.900, lab, ha="center", va="top", fontsize=7.5, color=c, fontweight="bold")

    fig.text(0.08, 0.868, f"All {n_total} brands, split by whether they beat, matched,\nor missed the industry average.",
              fontsize=7, color="#666666", va="top")

    fig.text(0.08, 0.822, "\u2713  TOP 5 MOST RELIABLE", fontsize=8.5, fontweight="bold", color=GREEN)
    ax1 = fig.add_axes([0.34, 0.628, 0.60, 0.178])

    fig.text(0.08, 0.605, "\u26a0  BOTTOM 5 LEAST RELIABLE", fontsize=8.5, fontweight="bold", color=RED)
    ax2 = fig.add_axes([0.34, 0.385, 0.60, 0.205])

    fig.text(0.08, 0.362, f"REMAINING {len(middle19)} BRANDS", fontsize=8.5, fontweight="bold", color="#444444")
    ax3 = fig.add_axes([0.08, 0.155, 0.86, 0.185])

    fig.text(0.5, 0.118, f"Cluster closely, from {mid_lo} to {mid_hi} PP100.\nFull list in the desktop version.",
              fontsize=6.8, color="#666666", ha="center", va="top")

    fig.text(0.5, 0.02, "Source: J.D. Power 2024 U.S. Vehicle Dependability Study.",
              fontsize=6, color="#888888", ha="center")

    def mini_lollipop(ax, rows, color, worst_on_right):
        ax.set_facecolor(BG)
        names = [r[0] for r in rows]
        vals = [r[1] for r in rows]
        diffs = [v - BASELINE for v in vals]
        yp = range(len(rows))
        for y, d, c in zip(yp, diffs, [color] * len(rows)):
            ax.hlines(y, 0, d, color=c, linewidth=2.0, zorder=3)
        ax.scatter(diffs, list(yp), color=color, s=26, zorder=4, edgecolor="white", linewidth=0.6)
        ax.invert_yaxis()
        ax.set_yticks(list(yp))
        ax.set_yticklabels(names, fontsize=7, fontweight="bold")
        # same "{diff} (value)" encoding as the full-size chart
        for y, d, v in zip(yp, diffs, vals):
            lab = f"{d} ({v})" if d == 0 else f"{d:+d} ({v})"
            if worst_on_right:
                ax.text(d + 3, y, lab, va="center", ha="left", fontsize=6.6, color=color, fontweight="bold")
            else:
                ax.text(d - 3, y, lab, va="center", ha="right", fontsize=6.6, color=color, fontweight="bold")
        ax.axvline(0, color="#222222", linestyle="--", linewidth=0.9, zorder=2)
        lim = max(abs(min(diffs)), abs(max(diffs))) * 1.55 + 12
        ax.set_xlim(-lim if not worst_on_right else -10, lim if worst_on_right else lim)
        ax.set_xticks([])
        for spine in ax.spines.values():
            spine.set_visible(False)
        ax.tick_params(length=0)

    mini_lollipop(ax1, top5, GREEN, worst_on_right=False)
    mini_lollipop(ax2, bottom5, RED, worst_on_right=True)

    # ---- Middle 19: compressed, unlabeled distribution strip ----
    ax3.set_facecolor(BG)
    mid_diffs = [v - BASELINE for v in mid_values]
    mid_colors = [color_for(d) for d in mid_diffs]
    ax3.scatter(mid_diffs, [0] * len(mid_diffs), color=mid_colors, s=22, alpha=0.85, zorder=3,
                edgecolor="white", linewidth=0.4)
    ax3.axvline(0, color="#222222", linestyle="--", linewidth=0.9, zorder=2)
    ax3.set_ylim(-0.6, 0.6)
    pad = (mid_hi - mid_lo) * 0.25
    ax3.set_xlim(mid_lo - BASELINE - pad, mid_hi - BASELINE + pad)
    ax3.set_yticks([])
    ax3.set_xticks([])
    for spine in ax3.spines.values():
        spine.set_visible(False)

    plt.savefig(path, dpi=DPI, facecolor=fig.get_facecolor())
    plt.close(fig)
    print(f"done: saved to {path}")


if __name__ == "__main__":
    build_fullsize()
    build_mobile()