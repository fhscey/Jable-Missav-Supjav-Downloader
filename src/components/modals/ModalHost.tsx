import React from 'react';
import { useUIStore } from '../../store/uiStore';
import { DetailModal } from './DetailModal';
import { PlayerModal } from './PlayerModal';
import { SettingsDrawer } from './SettingsDrawer';
import { SiteNavDrawer } from './SiteNavDrawer';
import { UpdateModal } from './UpdateModal';
import { DownloadPopover } from '../download/DownloadPopover';

/**
 * 全局弹窗与抽屉集中宿主组件
 * 负责统一订阅 uiStore 派发，避免在 Layout 中分散条件渲染导致主页面冗余重绘
 */
export const ModalHost: React.FC = () => {
  const activeDrawer = useUIStore((s) => s.activeDrawer);
  const closeDrawer = useUIStore((s) => s.closeDrawer);

  const selectedCard = useUIStore((s) => s.selectedCard);
  const selectedCardRect = useUIStore((s) => s.selectedCardRect);
  const closeDetail = useUIStore((s) => s.closeDetail);

  const playingMedia = useUIStore((s) => s.playingMedia);
  const playingCard = useUIStore((s) => s.playingCard);
  const playMedia = useUIStore((s) => s.playMedia);
  const closePlayer = useUIStore((s) => s.closePlayer);

  return (
    <>
      {activeDrawer === 'siteNav' && <SiteNavDrawer onClose={closeDrawer} />}
      {activeDrawer === 'settings' && <SettingsDrawer onClose={closeDrawer} />}
      {activeDrawer === 'downloads' && <DownloadPopover onClose={closeDrawer} />}

      {selectedCard && (
        <DetailModal
          card={selectedCard}
          anchorRect={selectedCardRect}
          onClose={closeDetail}
          onPlay={(detail) => playMedia(detail, selectedCard)}
        />
      )}

      {(playingMedia || playingCard) && (
        <PlayerModal detail={playingMedia} card={playingCard} onClose={closePlayer} />
      )}

      <UpdateModal />
    </>
  );
};
