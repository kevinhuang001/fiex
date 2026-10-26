import { t } from '@/i18n'
import { Choice, Label, Menu } from '@/components/Menus'
import { Button } from '@/components/ui/button'
import type { AppController } from '@/hooks/useAppController'
import type { Group, Sort, View } from '@/lib/types'
import { ArrowsDownUpIcon as ArrowsDownUp } from '@phosphor-icons/react/dist/csr/ArrowsDownUp'
import { CardsThreeIcon as CardsThree } from '@phosphor-icons/react/dist/csr/CardsThree'
import { ColumnsIcon as Columns3 } from '@phosphor-icons/react/dist/csr/Columns'
import { SquaresFourIcon as Grid2X2 } from '@phosphor-icons/react/dist/csr/SquaresFour'
import { ImagesIcon as Image } from '@phosphor-icons/react/dist/csr/Images'
import { RowsIcon as List } from '@phosphor-icons/react/dist/csr/Rows'
import type { ReactNode } from 'react'

const views: { id: View; name: string; icon: ReactNode }[] = [
  {
    id: 'icons',
    get name() {
      return t('图标')
    },
    icon: <Grid2X2 weight="regular" size={20} />,
  },
  {
    id: 'list',
    get name() {
      return t('列表')
    },
    icon: <List weight="regular" size={20} />,
  },
  {
    id: 'columns',
    get name() {
      return t('分栏')
    },
    icon: <Columns3 weight="regular" size={20} />,
  },
  {
    id: 'gallery',
    get name() {
      return t('画廊')
    },
    icon: <Image weight="regular" size={20} />,
  },
]

const groupOptions: [Group, string][] = [
  ['none', '无'],
  ['kind', '种类'],
  ['modified', '修改时间'],
  ['extension', '文件扩展名'],
  ['size', '文件大小'],
]

const sortOptions: [Sort, string][] = [
  ['name', '名称'],
  ['modified', '修改时间'],
  ['size', '大小'],
  ['kind', '种类'],
]

type ViewControlsProps = {
  prefs: Pick<AppController['prefs'], 'view' | 'group' | 'sort' | 'descending'>
  update: AppController['update']
}

export function ViewSwitch({ prefs, update }: ViewControlsProps) {
  return (
    <div className="view-switch">
      {views.map((v) => (
        <Button
          key={v.id}
          variant="ghost"
          size="icon"
          title={v.name}
          aria-label={t('{0}视图', v.name)}
          aria-pressed={prefs.view === v.id}
          onClick={() => update({ view: v.id })}
        >
          {v.icon}
        </Button>
      ))}
    </div>
  )
}

export function GroupControl({ prefs, update }: ViewControlsProps) {
  return (
    <Menu label={t('分组方式')} icon={<CardsThree weight="regular" size={20} />}>
      <Label>{t('分组方式')}</Label>
      {groupOptions.map(([id, name]) => (
        <Choice key={id} checked={prefs.group === id} onClick={() => update({ group: id })}>
          {t(name)}
        </Choice>
      ))}
    </Menu>
  )
}

export function SortControl({ prefs, update }: ViewControlsProps) {
  return (
    <Menu label={t('排序方式')} icon={<ArrowsDownUp weight="regular" size={20} />}>
      <Label>{t('排序方式')}</Label>
      {sortOptions.map(([id, name]) => (
        <Choice key={id} checked={prefs.sort === id} onClick={() => update({ sort: id })}>
          {t(name)}
        </Choice>
      ))}
      <Choice checked={prefs.descending} onClick={() => update({ descending: !prefs.descending })}>
        {t('降序')}
      </Choice>
    </Menu>
  )
}
