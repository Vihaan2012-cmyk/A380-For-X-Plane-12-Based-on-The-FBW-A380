//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0
import {
  BusSignal,
  Bus,
  UI,
  Derived,
  VarUnit,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import {
  DEFERRED_PROCEDURE_TYPE_TO_STRING,
  DeferredProcedureType,
  EcamDeferredProcedures,
  EcamInfos,
  EcamInopSys,
  EcamLimitations,
} from '../../../MsfsAvionicsCommon/EcamMessages';
import { FormattedFwcText } from '../../../EWD/elements/FormattedFwcText';
import { ChecklistState, FwsEvents } from '../../../MsfsAvionicsCommon/providers/FwsPublisher';
import { MoreLabel, PageTitle } from '../Generic/PageTitle';
import { SDSimvars } from '../../SDSimvarPublisher';
import { SdPageProps } from '../../SD';

import './style.scss';
import { RegisteredSimVar } from '@flybywiresim/fbw-sdk';
import { SdSoftKey } from './elements/SdSoftKey';

export const SD_STS_LINES_PER_PAGE = 17;
export const SD_STS_LINES_PER_PAGE_USABLE = 13; // Minus heading and footer

enum StatusPageSectionDisplayStatus {
  HIDDEN,
  VISIBLE_CURRENT_PAGE,
  VISIBLE_NEXT_PAGE,
}
export class StatusPage extends DestroyableComponent<SdPageProps> {
  private readonly sub = this.props.bus.inbox<SDSimvars & FwsEvents>();

  private readonly availChecker = new FwsSdAvailabilityChecker(this.props.bus);

  private readonly topSvgDisplay = this.props.visible.derive((v) => (v ? 'flex' : 'none'));

  private readonly stsPageToShow = BusSignal.from(this.sub.topic('sdStsPageToShow'), 0);

  private readonly statusNormal = Signal.of(true);

  private readonly stsNumberOfPagesSimvar = RegisteredSimVar.create(
    'L:A32NX_ECAM_SD_STS_NUMBER_OF_PAGES',
    VarUnit.Number,
  );
  private readonly stsMoreAvailableSimvar = RegisteredSimVar.create<number>(
    'L:A32NX_ECAM_SD_STS_MORE_AVAILABLE',
    VarUnit.Number,
  );

  /* LIMITATIONS */
  private readonly limitationsDisplayStatus = Signal.of<StatusPageSectionDisplayStatus>(
    StatusPageSectionDisplayStatus.HIDDEN,
  );

  private readonly limitationsAllPhases = BusSignal.from(this.sub.topic('fws_limitations_all_phases'), []);
  private readonly limitationsApprLdg = BusSignal.from(this.sub.topic('fws_limitations_appr_ldg'), []);

  private readonly limitationsLeftFormatString = this.limitationsAllPhases.derive((limits) =>
    limits.map((val) => EcamLimitations[val]).join('\r'),
  );

  private readonly limitationsRightFormatString = this.limitationsApprLdg.derive((limits) =>
    limits.map((val) => EcamLimitations[val]).join('\r'),
  );

  private readonly limitationsLines = Derived.from(
    ([all, apprLdg]) => (all.length > 0 || apprLdg.length > 0 ? Math.max(all.length, apprLdg.length) : 0),
    this.limitationsAllPhases,
    this.limitationsApprLdg,
  );
  private readonly limitationsDisplay = Derived.from(
    ([displayStatus, lines]) =>
      lines > 0 && displayStatus === StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE ? 'flex' : 'none',
    this.limitationsDisplayStatus,
    this.limitationsLines,
  );
  private readonly limitationsHeight = this.limitationsLines.derive((lines) => `${lines * 30 + 3}px`);

  /* DEFERRED PROCEDURE LIST */
  private readonly deferredProceduresDisplayStatus = Signal.of<StatusPageSectionDisplayStatus>(
    StatusPageSectionDisplayStatus.HIDDEN,
  );
  private readonly deferredProceduresOnNextPageDisplay = this.deferredProceduresDisplayStatus.derive((status) =>
    status === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE ? 'flex' : 'none',
  );
  private readonly deferredProcedures = BusSignal.from(this.sub.topic('fws_deferred_procedures'), []);

  private deferredProceduresLinesStrings: string[] = [];
  private readonly deferredProceduresFormatString = this.deferredProcedures.derive((proc) => {
    StatusPage.DeferredProcedureLines(proc, this.deferredProceduresLinesStrings);
    return this.deferredProceduresLinesStrings.join('\r');
  });

  private readonly deferredProceduresLines = Derived.from(
    ([proc]) => (proc.length > 0 ? proc.length * 2 : 0),
    this.deferredProcedures,
  );
  private readonly deferredProceduresDisplay = Derived.from(
    ([displayStatus, lines]) =>
      lines > 0 && displayStatus === StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE ? 'flex' : 'none',
    this.deferredProceduresDisplayStatus,
    this.deferredProceduresLines,
  );

  private readonly deferredProceduresDisplaySeparationLine = Signal.of(true);

  private readonly deferredProceduresHeight = this.deferredProceduresLines.derive((lines) => `${lines * 30 + 3}px`);

  /* INFO */
  private readonly infoDisplayStatus = Signal.of<StatusPageSectionDisplayStatus>(
    StatusPageSectionDisplayStatus.HIDDEN,
  );
  private readonly infoOnNextPageDisplay = this.infoDisplayStatus.derive((status) =>
    status === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE ? 'flex' : 'none',
  );

  private readonly info = BusSignal.from(this.sub.topic('fws_information'), []);

  private readonly infoFormatString = this.info.derive((info) => info.map((val) => EcamInfos[val]).join('\r'));

  private readonly infoLines = Derived.from(([info]) => (info.length > 0 ? info.length : 0), this.info);
  private readonly infoDisplay = Derived.from(
    ([displayStatus, lines]) =>
      lines > 0 && displayStatus === StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE ? 'flex' : 'none',
    this.infoDisplayStatus,
    this.infoLines,
  );

  private readonly infoDisplaySeparationLine = Signal.of(true);

  private readonly infoHeight = this.infoLines.derive((lines) => `${lines * 30 + 3}px`);

  /* INOP SYS */
  private readonly inopSysDisplayStatus = Signal.of<StatusPageSectionDisplayStatus>(
    StatusPageSectionDisplayStatus.HIDDEN,
  );
  private readonly inopSysOnNextPageDisplay = this.inopSysDisplayStatus.derive((status) =>
    status === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE ? 'flex' : 'none',
  );

  private readonly inopSysAllPhases = BusSignal.from(this.sub.topic('fws_inop_sys_all_phases'), []);
  private readonly inopSysApprLdg = BusSignal.from(this.sub.topic('fws_inop_sys_appr_ldg'), []);

  private readonly inopSysLeftFormatString = this.inopSysAllPhases.derive((limits) =>
    limits.map((val) => EcamInopSys[val]).join('\r'),
  );

  private readonly inopSysRightFormatString = this.inopSysApprLdg.derive((limits) =>
    limits.map((val) => EcamInopSys[val]).join('\r'),
  );

  private readonly inopSysLines = Derived.from(
    ([all, apprLdg]) => (all.length > 0 || apprLdg.length > 0 ? Math.max(all.length, apprLdg.length) : 0),
    this.inopSysAllPhases,
    this.inopSysApprLdg,
  );
  private readonly inopSysDisplay = Derived.from(
    ([displayStatus, lines]) =>
      lines > 0 && displayStatus === StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE ? 'flex' : 'none',
    this.inopSysDisplayStatus,
    this.inopSysLines,
  );

  private readonly inopSysDisplaySeparationLine = Signal.of(true);

  private readonly inopSysHeight = this.inopSysLines.derive((lines) => `${lines * 30 + 3}px`);

  /* ALERTS IMPACTING LDG PERF, TODO */
  private readonly alertImpactingLdgPerfDisplayStatus = Signal.of<StatusPageSectionDisplayStatus>(
    StatusPageSectionDisplayStatus.HIDDEN,
  );

  /* INOP SYS REDUND */
  private readonly inopSysRedund = BusSignal.from(this.sub.topic('fws_inop_sys_redundancy_loss'), []);
  private readonly inopSysRedundCurrentPage = Derived.from(
    ([inopSysRedund, pageToShow]) =>
      inopSysRedund.slice(pageToShow * SD_STS_LINES_PER_PAGE_USABLE, (pageToShow + 1) * SD_STS_LINES_PER_PAGE_USABLE),
    this.inopSysRedund,
    this.stsPageToShow,
  );

  private readonly inopSysRedundFormatString = this.inopSysRedundCurrentPage.derive((inopSysRedundCurrentPage) =>
    inopSysRedundCurrentPage.map((val) => EcamInopSys[val]).join('\r'),
  );

  private readonly inopSysRedundLines = Derived.from(
    ([inopSysRedund]) => (inopSysRedund.length > 0 ? inopSysRedund.length : 0),
    this.inopSysRedundCurrentPage,
  );
  private readonly inopSysRedundDisplay = Derived.from(
    ([lines]) => (lines > 0 ? 'flex' : 'none'),
    this.inopSysRedundLines,
  );

  private readonly inopSysRedundHeight = this.inopSysRedundLines.derive((lines) => `${lines * 30 + 3}px`);

  private readonly moreActive = BusSignal.from(this.sub.topic('moreActive'), false);
  private readonly moreAvailable = this.inopSysRedund.derive((lines) => lines.length > 0);
  private readonly moreAvailableVisibility = this.moreAvailable.derive((v) => (v ? 'inherit' : 'hidden'));
  private readonly moreActiveVisibility = this.moreActive.derive((v) => (v ? 'inherit' : 'hidden'));

  private readonly statusNormalDisplay = Derived.from(
    ([normal, notAvail]) => (normal && !notAvail ? 'flex' : 'none'),
    this.statusNormal,
    this.availChecker.fwsFailed,
  );
  private readonly statusNotAvailableDisplay = Derived.from(
    ([notAvail]) => (notAvail ? 'flex' : 'none'),
    this.availChecker.fwsFailed,
  );
  private readonly abnormalContentDisplay = Derived.from(
    ([normal, notAvail, moreActive]) => (!normal && !notAvail && !moreActive ? 'flex' : 'none'),
    this.statusNormal,
    this.availChecker.fwsFailed,
    this.moreActive,
  );
  private readonly abnormalContentMoreDisplay = Derived.from(
    ([normal, notAvail, moreActive]) => (!normal && !notAvail && moreActive ? 'flex' : 'none'),
    this.statusNormal,
    this.availChecker.fwsFailed,
    this.moreActive,
  );

  private readonly pressStsForNextStatusPageVisibility = Derived.from(
    ([defStatus, infoStatus, inopStatus, alertStatus]) =>
      defStatus === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE ||
      infoStatus === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE ||
      inopStatus === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE ||
      alertStatus === StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE
        ? 'inherit'
        : 'hidden',
    this.deferredProceduresDisplayStatus,
    this.infoDisplayStatus,
    this.inopSysDisplayStatus,
    this.alertImpactingLdgPerfDisplayStatus,
  );

  private readonly morePageOverflow = Derived.from(
    ([inopSysRedund, pageToShow]) => inopSysRedund.length > (pageToShow + 1) * SD_STS_LINES_PER_PAGE_USABLE,
    this.inopSysRedund,
    this.stsPageToShow,
  );
  private readonly pressMoreForNextMorePageVisibility = this.morePageOverflow.derive((v) => (v ? 'inherit' : 'hidden'));

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.topSvgDisplay,
      this.limitationsAllPhases,
      this.limitationsApprLdg,
      this.limitationsLeftFormatString,
      this.limitationsRightFormatString,
      this.limitationsLines,
      this.limitationsDisplay,
      this.deferredProceduresOnNextPageDisplay,
      this.deferredProcedures,
      this.deferredProceduresFormatString,
      this.deferredProceduresLines,
      this.deferredProceduresDisplay,
      this.infoOnNextPageDisplay,
      this.info,
      this.infoFormatString,
      this.infoLines,
      this.infoDisplay,
      this.infoHeight,
      this.inopSysOnNextPageDisplay,
      this.inopSysAllPhases,
      this.inopSysApprLdg,
      this.inopSysLeftFormatString,
      this.inopSysRightFormatString,
      this.inopSysLines,
      this.inopSysDisplay,
      this.inopSysHeight,
      this.pressStsForNextStatusPageVisibility,
    );

    this.subscriptions.push(
      Derived.from(
        ([pageToShow, limAll, limAppr, deferredProcedures, info, inopApp, inopAppr, moreActive, inopSysRedund]) => {
          // Calculate number of pages required and where to display "ON NEXT PAGE" messages
          // 6 lines reserved for heading and white PRESS STS... line
          if (
            limAll.length === 0 &&
            limAppr.length === 0 &&
            deferredProcedures.length === 0 &&
            info.length === 0 &&
            inopApp.length === 0 &&
            inopAppr.length === 0
          ) {
            this.statusNormal.set(true);
          } else {
            this.statusNormal.set(false);
          }

          if (moreActive) {
            this.stsNumberOfPagesSimvar.set(
              inopSysRedund.length > 0 ? Math.ceil(inopSysRedund.length / SD_STS_LINES_PER_PAGE_USABLE) : 1,
            );
            return;
          }

          // Add one or two lines for header
          const limitationsLines = Math.max(limAll.length, limAppr.length) + 2;
          const deferredProceduresLines = this.deferredProceduresLinesStrings.length + 1; // Cave: Check if race condition
          const infoLines = info.length + 1;
          const inopSysLines = Math.max(inopApp.length, inopAppr.length) + 2;
          const alertImpactingLdgPerfLines = 0; // TODO

          // 0-based page index. -1 means not displayed
          let deferredProceduresVisibleOnPage = 0;
          let infoVisibleOnPage = 0;
          let inopSysVisibleOnPage = 0;
          let alertImpactingLdgPerfVisibleOnPage = 0; // TODO

          let calculateForPage = 0;
          let linesUsedOnCurrentPage = 0;

          // LIMITATIONS, always on first page
          linesUsedOnCurrentPage += limitationsLines;
          this.limitationsDisplayStatus.set(
            limitationsLines > 0 && pageToShow === 0
              ? StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE
              : StatusPageSectionDisplayStatus.HIDDEN,
          );

          if (linesUsedOnCurrentPage + deferredProceduresLines > SD_STS_LINES_PER_PAGE_USABLE) {
            deferredProceduresVisibleOnPage = calculateForPage + 1;
            calculateForPage += 1;
            linesUsedOnCurrentPage = 0;
            this.deferredProceduresDisplaySeparationLine.set(false);
          } else {
            deferredProceduresVisibleOnPage = calculateForPage;
            this.deferredProceduresDisplaySeparationLine.set(true);
          }
          linesUsedOnCurrentPage += deferredProceduresLines;

          if (linesUsedOnCurrentPage + infoLines > SD_STS_LINES_PER_PAGE_USABLE) {
            infoVisibleOnPage = calculateForPage + 1;
            calculateForPage += 1;
            linesUsedOnCurrentPage = 0;
            this.infoDisplaySeparationLine.set(false);
          } else {
            infoVisibleOnPage = calculateForPage;
            this.infoDisplaySeparationLine.set(true);
          }
          linesUsedOnCurrentPage += infoLines;

          if (linesUsedOnCurrentPage + inopSysLines > SD_STS_LINES_PER_PAGE_USABLE) {
            inopSysVisibleOnPage = calculateForPage + 1;
            calculateForPage += 1;
            linesUsedOnCurrentPage = 0;
            this.inopSysDisplaySeparationLine.set(false);
          } else {
            inopSysVisibleOnPage = calculateForPage;
            this.inopSysDisplaySeparationLine.set(true);
          }
          linesUsedOnCurrentPage += inopSysLines;

          if (linesUsedOnCurrentPage + alertImpactingLdgPerfLines > SD_STS_LINES_PER_PAGE_USABLE) {
            alertImpactingLdgPerfVisibleOnPage = calculateForPage + 1;
            calculateForPage += 1;
            linesUsedOnCurrentPage = 0;
          } else {
            alertImpactingLdgPerfVisibleOnPage = calculateForPage;
          }
          linesUsedOnCurrentPage += alertImpactingLdgPerfLines;

          let displayedOnNextPageAlreadyDisplayed = false;
          if (pageToShow === deferredProceduresVisibleOnPage) {
            this.deferredProceduresDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE);
          } else if (pageToShow + 1 === deferredProceduresVisibleOnPage && !displayedOnNextPageAlreadyDisplayed) {
            this.deferredProceduresDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE);
            displayedOnNextPageAlreadyDisplayed = true;
          } else {
            this.deferredProceduresDisplayStatus.set(StatusPageSectionDisplayStatus.HIDDEN);
          }

          if (pageToShow === infoVisibleOnPage) {
            this.infoDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE);
          } else if (pageToShow + 1 === infoVisibleOnPage && !displayedOnNextPageAlreadyDisplayed) {
            this.infoDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE);
            displayedOnNextPageAlreadyDisplayed = true;
          } else {
            this.infoDisplayStatus.set(StatusPageSectionDisplayStatus.HIDDEN);
          }

          if (pageToShow === inopSysVisibleOnPage) {
            this.inopSysDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE);
          } else if (pageToShow + 1 === inopSysVisibleOnPage && !displayedOnNextPageAlreadyDisplayed) {
            this.inopSysDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE);
            displayedOnNextPageAlreadyDisplayed = true;
          } else {
            this.inopSysDisplayStatus.set(StatusPageSectionDisplayStatus.HIDDEN);
          }

          if (pageToShow === alertImpactingLdgPerfVisibleOnPage) {
            this.alertImpactingLdgPerfDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_CURRENT_PAGE);
          } else if (pageToShow + 1 === alertImpactingLdgPerfVisibleOnPage && !displayedOnNextPageAlreadyDisplayed) {
            this.alertImpactingLdgPerfDisplayStatus.set(StatusPageSectionDisplayStatus.VISIBLE_NEXT_PAGE);
            displayedOnNextPageAlreadyDisplayed = true;
          } else {
            this.alertImpactingLdgPerfDisplayStatus.set(StatusPageSectionDisplayStatus.HIDDEN);
          }

          this.stsNumberOfPagesSimvar.set(calculateForPage + 1);
        },
        this.stsPageToShow,
        this.limitationsAllPhases,
        this.limitationsApprLdg,
        this.deferredProceduresFormatString,
        this.info,
        this.inopSysAllPhases,
        this.inopSysApprLdg,
        this.moreActive,
        this.inopSysRedund,
      ),
      this.moreAvailable.watch((v) => {
        this.stsMoreAvailableSimvar.set(v ? 1 : 0);
      }, true),
    );
  }

  destroy(): void {
    super.destroy();
  }

  render() {
    return (
      <div id="sts" class="sd-sts-top-div" style={{ display: this.topSvgDisplay }}>
        <svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlnsXlink="http://www.w3.org/1999/xlink" height="45">
          <PageTitle x={6} y={29}>
            STATUS
          </PageTitle>
          <g visibility={this.moreActiveVisibility}>
            <MoreLabel x={187} y={28} moreActive={this.moreActive} />
          </g>
        </svg>
        <div id="sts-normal-content" class="sd-sts-centered-text" style={{ display: this.statusNormalDisplay }}>
          NORMAL
        </div>
        <div
          id="sts-normal-content"
          class="sd-sts-centered-text amber"
          style={{ display: this.statusNotAvailableDisplay }}
        >
          STATUS NOT AVAILABLE
        </div>
        <div id="sts-abnormal-content" class="sd-sts-abnormal-content" style={{ display: this.abnormalContentDisplay }}>
          {/* LIMITATIONS */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.limitationsDisplay,
            }}
          >
            <StatusPageSectionHeading title="LIMITATIONS" showSeparationLines={Signal.of(false)} />
            <div class="sd-sts-section-divided-area">
              <div class="sd-sts-section-divided-left">
                <span class="sd-sts-section-divided-area-heading">ALL PHASES</span>
                <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.limitationsHeight }}>
                  <FormattedFwcText x={0} y={24} message={this.limitationsLeftFormatString} />
                </svg>
              </div>
              <div class="sd-sts-section-divided-right">
                <span class="sd-sts-section-divided-area-heading">APPR & LDG</span>
                <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.limitationsHeight }}>
                  <FormattedFwcText x={0} y={24} message={this.limitationsRightFormatString} />
                </svg>
              </div>
            </div>
          </div>
          {/* DEFERRED PROCEDURE LIST on next page */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.deferredProceduresOnNextPageDisplay,
            }}
          >
            <StatusPageSectionHeading title="DEFERRED PROCEDURE LIST" showSeparationLines={Signal.of(true)} />
            <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: '63px' }}>
              <FormattedFwcText x={0} y={24} message={'\x1b<4mDISPLAYED ON NEXT STATUS PAGE...'} />
            </svg>
          </div>
          {/* DEFERRED PROCEDURE LIST */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.deferredProceduresDisplay,
            }}
          >
            <StatusPageSectionHeading
              title="DEFERRED PROCEDURE LIST"
              showSeparationLines={this.deferredProceduresDisplaySeparationLine}
            />
            <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.deferredProceduresHeight }}>
              <FormattedFwcText x={0} y={24} message={this.deferredProceduresFormatString} />
            </svg>
          </div>
          {/* INFO on next page */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.infoOnNextPageDisplay,
            }}
          >
            <StatusPageSectionHeading title="INFO" showSeparationLines={Signal.of(true)} />
            <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: '63px' }}>
              <FormattedFwcText x={0} y={24} message={'\x1b<3mDISPLAYED ON NEXT STATUS PAGE...'} />
            </svg>
          </div>
          {/* INFO */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.infoDisplay,
            }}
          >
            <StatusPageSectionHeading title="INFO" showSeparationLines={this.infoDisplaySeparationLine} />
            <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.infoHeight }}>
              <FormattedFwcText x={0} y={24} message={this.infoFormatString} />
            </svg>
          </div>
          {/* INOP SYS on next page */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.inopSysOnNextPageDisplay,
            }}
          >
            <StatusPageSectionHeading title="INOP SYS" showSeparationLines={Signal.of(true)} />
            <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: '63px' }}>
              <FormattedFwcText x={0} y={24} message={'\x1b<4mDISPLAYED ON NEXT STATUS PAGE...'} />
            </svg>
          </div>
          {/* INOP SYS */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.inopSysDisplay,
            }}
          >
            <StatusPageSectionHeading title="INOP SYS" showSeparationLines={this.inopSysDisplaySeparationLine} />
            <div class="sd-sts-section-divided-area">
              <div class="sd-sts-section-divided-left">
                <span class="sd-sts-section-divided-area-heading">ALL PHASES</span>
                <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.inopSysHeight }}>
                  <FormattedFwcText x={0} y={24} message={this.inopSysLeftFormatString} />
                </svg>
              </div>
              <div class="sd-sts-section-divided-right">
                <span class="sd-sts-section-divided-area-heading">APPR & LDG</span>
                <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.inopSysHeight }}>
                  <FormattedFwcText x={0} y={24} message={this.inopSysRightFormatString} />
                </svg>
              </div>
            </div>
          </div>
          <div style="flex-grow: 1" />
          <div class="sd-sts-press-sts-for-next-page" style={{ visibility: this.pressStsForNextStatusPageVisibility }}>
            PRESS STS FOR NEXT STATUS PAGE
          </div>
          <div class="sd-sts-bottom-area">
            <div
              class="sd-sts-bottom-area-more-box"
              style={{
                visibility: this.moreAvailableVisibility,
              }}
            >
              MORE
            </div>
          </div>
          <SdSoftKey />
        </div>
        <div
          id="sts-abnormal-content-more"
          class="sd-sts-abnormal-content"
          style={{ display: this.abnormalContentMoreDisplay }}
        >
          {/* INOP SYS REDUND */}
          <div
            class="sd-sts-section-container"
            style={{
              display: this.inopSysRedundDisplay,
            }}
          >
            <StatusPageSectionHeading title="INOP SYS REDUND" showSeparationLines={Signal.of(false)} />
            <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.inopSysRedundHeight }}>
              <FormattedFwcText x={0} y={24} message={this.inopSysRedundFormatString} />
            </svg>
          </div>
          <div style="flex-grow: 1" />
          <div class="sd-sts-press-sts-for-next-page" style={{ visibility: this.pressMoreForNextMorePageVisibility }}>
            PRESS MORE FOR NEXT MORE PAGE
          </div>
          <SdSoftKey />
        </div>
      </div>
    );
  }

  /**
   * Returns an array, each line one element, with FwcFormattedString capable strings
   * @param proceduresState checklistState from FWS
   * @param outputLines array to fill with output lines to avoid allocations
   */
  public static DeferredProcedureLines(proceduresState: readonly ChecklistState[], outputLines: string[]): void {
    outputLines.length = 0;
    if (proceduresState.length === 0) {
      return;
    }

    Object.keys(DeferredProcedureType).forEach((_, procTypeIndex) => {
      const procsOfType = proceduresState.filter(
        (val) => EcamDeferredProcedures[val.id].type === Number(procTypeIndex),
      );
      if (procsOfType.length > 0) {
        outputLines.push(`\x1b<4m${DEFERRED_PROCEDURE_TYPE_TO_STRING[procTypeIndex]} :`);
        procsOfType.forEach((val) => {
          outputLines.push(`> \x1b<4m${EcamDeferredProcedures[val.id].title}`);
        });
      }
    });
  }
}

interface StatusPageSectionHeadingProps {
  title: string;
  showSeparationLines: Readable<boolean>;
}
export class StatusPageSectionHeading extends DestroyableComponent<StatusPageSectionHeadingProps> {
  render() {
    return (
      <div class="sd-sts-section-heading-group">
        <div class={{ 'sd-sts-section-heading-lines': true, show: this.props.showSeparationLines }} />
        <span class="sd-sts-section-heading underline">{this.props.title}</span>
        <div class={{ 'sd-sts-section-heading-lines': true, show: this.props.showSeparationLines }} />
      </div>
    );
  }
}

class FwsSdAvailabilityChecker {
  constructor(private bus: Bus) {}

  private readonly sub = this.bus.inbox<SDSimvars>();

  private readonly fws1IsHealthy = BusSignal.from(this.sub.topic('fws1_is_healthy'), true);
  private readonly fws2IsHealthy = BusSignal.from(this.sub.topic('fws2_is_healthy'), true);

  private readonly afdx_4_4_reachable = BusSignal.from(this.sub.topic('afdx_4_4_reachable'), true);
  private readonly afdx_14_14_reachable = BusSignal.from(this.sub.topic('afdx_14_14_reachable'), true);
  private readonly afdx_4_3_reachable = BusSignal.from(this.sub.topic('afdx_4_3_reachable'), true);
  private readonly afdx_14_13_reachable = BusSignal.from(this.sub.topic('afdx_14_13_reachable'), true);

  public readonly fwsAvail = Derived.from(
    ([healthy1, healthy2, r_4_4, r_14_14, r_4_3, r_14_13]) =>
      (healthy1 && (r_4_3 || r_14_13)) || (healthy2 && (r_4_4 || r_14_14)),
    this.fws1IsHealthy,
    this.fws2IsHealthy,
    this.afdx_4_4_reachable,
    this.afdx_14_14_reachable,
    this.afdx_4_3_reachable,
    this.afdx_14_13_reachable,
  );

  public readonly fwsFailed = this.fwsAvail.derive((it) => !it);
}
