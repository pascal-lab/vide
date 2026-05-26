const FILE_STRIP_SCROLL_IDLE_MS = 850;

export interface FileStripScrollState {
  fileStripOverflowing: boolean;
  fileStripScrolling: boolean;
  fileStripDragging: boolean;
  fileStripThumbLeft: number;
  fileStripThumbWidth: number;
}

interface FileStripDragState {
  startClientX: number;
  startScrollLeft: number;
  maxScrollLeft: number;
  trackWidth: number;
  thumbWidth: number;
}

export class FileStripScrollController {
  private dragState: FileStripDragState | undefined;
  private scrollTimer: number | undefined;
  private measureFrame: number | undefined;

  readonly state: FileStripScrollState = {
    fileStripOverflowing: false,
    fileStripScrolling: false,
    fileStripDragging: false,
    fileStripThumbLeft: 0,
    fileStripThumbWidth: 100,
  };

  private readonly handleThumbDrag = (event: PointerEvent) => this.dragThumb(event);
  private readonly handleThumbRelease = () => this.endThumbDrag();

  constructor(
    private readonly root: () => ParentNode,
    private readonly invalidate: () => void,
  ) {}

  dispose(): void {
    this.clearTimers();
    this.removeDragListeners();
    this.dragState = undefined;
  }

  updateScroll(event: Event): void {
    if (event.currentTarget instanceof HTMLElement) {
      this.updateScrollbar(event.currentTarget, true);
    }
  }

  jumpScrollbar(event: PointerEvent): void {
    if (!this.state.fileStripOverflowing) {
      return;
    }
    event.preventDefault();

    const strip = this.fileStripElement();
    const track = this.scrollbarElement();
    if (!strip || !track) {
      return;
    }

    const rect = track.getBoundingClientRect();
    const maxScrollLeft = Math.max(0, strip.scrollWidth - strip.clientWidth);
    const thumbWidth = (this.state.fileStripThumbWidth / 100) * rect.width;
    const travelWidth = Math.max(1, rect.width - thumbWidth);
    const thumbLeft = clamp(event.clientX - rect.left - thumbWidth / 2, 0, travelWidth);
    strip.scrollLeft = (thumbLeft / travelWidth) * maxScrollLeft;
    this.updateScrollbar(strip, true);
  }

  beginThumbDrag(event: PointerEvent): void {
    if (!this.state.fileStripOverflowing) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();

    const strip = this.fileStripElement();
    const track = this.scrollbarElement();
    if (!strip || !track) {
      return;
    }

    const rect = track.getBoundingClientRect();
    const maxScrollLeft = Math.max(0, strip.scrollWidth - strip.clientWidth);
    if (maxScrollLeft <= 0 || rect.width <= 0) {
      return;
    }

    this.dragState = {
      startClientX: event.clientX,
      startScrollLeft: strip.scrollLeft,
      maxScrollLeft,
      trackWidth: rect.width,
      thumbWidth: (this.state.fileStripThumbWidth / 100) * rect.width,
    };
    this.state.fileStripDragging = true;
    this.state.fileStripScrolling = true;
    this.clearScrollTimer();
    this.addDragListeners();
    this.invalidate();
  }

  queueMeasurement(): void {
    if (this.measureFrame !== undefined || typeof window === "undefined") {
      return;
    }

    this.measureFrame = window.requestAnimationFrame(() => {
      this.measureFrame = undefined;
      const strip = this.fileStripElement();
      if (strip) {
        this.updateScrollbar(strip);
      }
    });
  }

  private dragThumb(event: PointerEvent): void {
    const drag = this.dragState;
    const strip = this.fileStripElement();
    if (!drag || !strip) {
      return;
    }

    const travelWidth = Math.max(1, drag.trackWidth - drag.thumbWidth);
    const delta = event.clientX - drag.startClientX;
    strip.scrollLeft = clamp(drag.startScrollLeft + (delta / travelWidth) * drag.maxScrollLeft, 0, drag.maxScrollLeft);
    this.updateScrollbar(strip, true);
  }

  private endThumbDrag(): void {
    if (!this.dragState) {
      return;
    }

    this.dragState = undefined;
    this.state.fileStripDragging = false;
    this.removeDragListeners();
    this.scheduleScrollbarFade();
    this.invalidate();
  }

  private updateScrollbar(strip: HTMLElement, reveal = false): void {
    const clientWidth = Math.max(0, strip.clientWidth);
    const scrollWidth = Math.max(clientWidth, strip.scrollWidth);
    const maxScrollLeft = Math.max(0, scrollWidth - clientWidth);
    const overflowing = maxScrollLeft > 1;
    const trackWidth = Math.max(1, clientWidth - 16);
    const minThumbWidth = Math.min(100, (20 / trackWidth) * 100);
    const thumbWidth = overflowing ? Math.min(100, Math.max(minThumbWidth, (clientWidth / scrollWidth) * 100)) : 100;
    const thumbTravel = Math.max(0, 100 - thumbWidth);
    const thumbLeft = overflowing && maxScrollLeft > 0 ? (strip.scrollLeft / maxScrollLeft) * thumbTravel : 0;

    const changed =
      this.state.fileStripOverflowing !== overflowing ||
      Math.abs(this.state.fileStripThumbWidth - thumbWidth) > 0.1 ||
      Math.abs(this.state.fileStripThumbLeft - thumbLeft) > 0.1;

    this.state.fileStripOverflowing = overflowing;
    this.state.fileStripThumbWidth = thumbWidth;
    this.state.fileStripThumbLeft = clamp(thumbLeft, 0, thumbTravel);

    if (!overflowing && this.state.fileStripScrolling) {
      this.clearScrollTimer();
      this.state.fileStripScrolling = false;
      this.state.fileStripDragging = false;
    }

    if (changed) {
      this.invalidate();
    }

    if (reveal && overflowing) {
      this.revealScrollbar();
    }
  }

  private revealScrollbar(): void {
    this.clearScrollTimer();
    if (!this.state.fileStripScrolling) {
      this.state.fileStripScrolling = true;
      this.invalidate();
    }
    this.scheduleScrollbarFade();
  }

  private scheduleScrollbarFade(): void {
    this.clearScrollTimer();
    this.scrollTimer = window.setTimeout(() => {
      this.scrollTimer = undefined;
      if (!this.state.fileStripDragging && this.state.fileStripScrolling) {
        this.state.fileStripScrolling = false;
        this.invalidate();
      }
    }, FILE_STRIP_SCROLL_IDLE_MS);
  }

  private clearTimers(): void {
    this.clearScrollTimer();
    if (this.measureFrame !== undefined) {
      window.cancelAnimationFrame(this.measureFrame);
      this.measureFrame = undefined;
    }
  }

  private clearScrollTimer(): void {
    if (this.scrollTimer !== undefined) {
      window.clearTimeout(this.scrollTimer);
      this.scrollTimer = undefined;
    }
  }

  private addDragListeners(): void {
    window.addEventListener("pointermove", this.handleThumbDrag);
    window.addEventListener("pointerup", this.handleThumbRelease, { once: true });
    window.addEventListener("pointercancel", this.handleThumbRelease, { once: true });
  }

  private removeDragListeners(): void {
    window.removeEventListener("pointermove", this.handleThumbDrag);
    window.removeEventListener("pointerup", this.handleThumbRelease);
    window.removeEventListener("pointercancel", this.handleThumbRelease);
  }

  private fileStripElement(): HTMLElement | undefined {
    return this.root().querySelector<HTMLElement>(".file-strip") ?? undefined;
  }

  private scrollbarElement(): HTMLElement | undefined {
    return this.root().querySelector<HTMLElement>(".file-strip-scrollbar") ?? undefined;
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
