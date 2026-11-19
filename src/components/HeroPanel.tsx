import { type CSSProperties, Fragment } from "react";
import fireSprite from "../assets/hero/camp_fire.png";
import goblinSprite from "../assets/hero/foe_goblin.png";
import golemSprite from "../assets/hero/foe_golem.png";
import ogreSprite from "../assets/hero/foe_ogre.png";
import poofSprite from "../assets/hero/foe_poof.png";
import slimeSprite from "../assets/hero/foe_slime.png";
import idleSprite from "../assets/hero/hero_idle.png";
import sitSprite from "../assets/hero/hero_sit.png";
import sleepSprite from "../assets/hero/hero_sleep.png";
import walkSprite from "../assets/hero/hero_walk.png";
import boltSprite from "../assets/hero/spell_bolt.png";
import campLayer from "../assets/hero/world_camp.png";
import cloudsLayer from "../assets/hero/world_clouds.png";
import groundLayer from "../assets/hero/world_ground.png";
import hillsLayer from "../assets/hero/world_hills.png";
import mountainsLayer from "../assets/hero/world_mountains.png";
import skyLayer from "../assets/hero/world_sky.png";
import type { ProgressionSnapshot, Reward, TimerPhase } from "../flowmodoro";
import {
  type FoeTier,
  foeTierFor,
  type HeroState,
  heroCaption,
  heroSceneFor,
  heroStateFor,
  levelProgressPercent,
  rewardLines,
  rewardTitle,
} from "../hero";

const sprites: Record<HeroState, string> = {
  idle: idleSprite,
  adventure: walkSprite,
  sleep: sleepSprite,
  camp: sitSprite,
};

const foes: Record<FoeTier, string> = {
  slime: slimeSprite,
  goblin: goblinSprite,
  ogre: ogreSprite,
  golem: golemSprite,
};

/** Parallax layers, back to front. Each wraps horizontally and scrolls at its own speed. */
const layers = [
  ["sky", skyLayer],
  ["clouds", cloudsLayer],
  ["mountains", mountainsLayer],
  ["hills", hillsLayer],
  ["ground", groundLayer],
] as const;

const image = (url: string): CSSProperties => ({ backgroundImage: `url(${url})` });

interface HeroPanelProps {
  phase: TimerPhase;
  /** Effective focus in the current block; bigger foes appear as it grows. */
  focusBlockMs: number;
  progression: ProgressionSnapshot | undefined;
  reward: Reward | null;
  onDismissReward(): void;
}

function HeroPanel({ phase, focusBlockMs, progression, reward, onDismissReward }: HeroPanelProps) {
  const state = heroStateFor(phase);
  const caption = heroCaption(state);
  const level = progression?.level ?? 1;
  const atMaxLevel = progression ? progression.xpForNextLevel === 0 : false;
  const percent = progression ? levelProgressPercent(progression) : 0;
  const scene = heroSceneFor(phase);

  return (
    <fieldset className="hero-panel">
      <legend>Herói</legend>
      <div
        className={`hero-stage hero-stage--${scene}${state === "adventure" ? " is-traveling" : ""}`}
        aria-hidden="true"
      >
        <div className="hero-world">
          {layers.map(([name, url]) => (
            <div key={name} className={`scene-layer scene-layer--${name}`} style={image(url)} />
          ))}
          {scene === "camp" && <div className="scene-layer scene-layer--camp" style={image(campLayer)} />}
        </div>
        {scene === "camp" && <div className="camp-fire" style={image(fireSprite)} />}
        {state === "adventure" && (
          // Keyed by tier so a bigger foe restarts the cycle instead of swapping mid-walk.
          <Fragment key={foeTierFor(focusBlockMs)}>
            <div className={`foe foe--${foeTierFor(focusBlockMs)}`} style={image(foes[foeTierFor(focusBlockMs)])} />
            <div className="spell-bolt" style={image(boltSprite)} />
            <div className={`foe-poof foe-poof--${foeTierFor(focusBlockMs)}`} style={image(poofSprite)} />
          </Fragment>
        )}
        <div className={`hero-sprite hero-sprite--${state}`} style={image(sprites[state])} />
      </div>
      <div className="hero-stats">
        <p className="hero-level"><strong>Nível {level}</strong> · {caption}</p>
        <div
          className="progress-indicator hero-progress"
          role="progressbar"
          aria-label={atMaxLevel ? "Nível máximo alcançado" : `Progresso para o nível ${level + 1}`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
        >
          <span className="progress-indicator-bar" style={{ width: `${percent}%` }} />
        </div>
        <p className="hero-xp" title="Ganhe XP finalizando focos e cumprindo o descanso.">
          {atMaxLevel
            ? `${progression?.totalXp ?? 0} XP · nível máximo`
            : `${progression?.xpIntoLevel ?? 0} / ${progression?.xpForNextLevel ?? 60} XP`}
        </p>
      </div>
      <div className="hero-reward" aria-live="polite">
        {reward && (
          <div className="window reward-notice" aria-labelledby="reward-title">
            <div className="title-bar">
              <div className="title-bar-text" id="reward-title">{rewardTitle(reward)}</div>
            </div>
            <div className="window-body">
              {rewardLines(reward).map((line) => <p key={line}>{line}</p>)}
              <button type="button" onClick={onDismissReward}>OK</button>
            </div>
          </div>
        )}
      </div>
    </fieldset>
  );
}

export default HeroPanel;
