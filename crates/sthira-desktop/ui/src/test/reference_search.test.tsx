import { act,fireEvent,render,screen,waitFor } from '@testing-library/react';
import { expect,it,vi } from 'vitest';
import { SearchModal } from '../components/SearchModal';
import { searchAll } from '../api/ipc';
vi.mock('../api/ipc',()=>({searchAll:vi.fn()}));
const a='sthira://artifact/9ea18c81-36ab-4111-ae5d-a6e0250011fd';
const b='sthira://artifact/1e6e33ee-987c-4854-b646-664b0d4fa3aa';
it('opens the exact reference result by keyboard after pasting its URL',async()=>{
 vi.mocked(searchAll).mockResolvedValue([{id:'target',title:'Renamed source',kind:'document',project_id:null,excerpt:'Saved workspace reference'}]);const open=vi.fn();render(<SearchModal isOpen onClose={vi.fn()} onOpenProject={vi.fn()} onOpenArtifact={open}/>);
 fireEvent.change(screen.getByLabelText('Search workspace entities'),{target:{value:a}});await screen.findByRole('option',{name:'Open Renamed source'});expect(searchAll).toHaveBeenCalledWith(a);fireEvent.keyDown(screen.getByLabelText('Search workspace entities'),{key:'Enter'});expect(open).toHaveBeenCalledWith('target');
});
it('ignores a late reference lookup after the user pastes a different URL',async()=>{
 let finish!:(value:any)=>void;vi.mocked(searchAll).mockImplementation(query=>query===a?new Promise(resolve=>{finish=resolve;}):Promise.resolve([{id:'b',title:'Executive Summary',kind:'document',project_id:null,excerpt:''}]));render(<SearchModal isOpen onClose={vi.fn()} onOpenProject={vi.fn()} onOpenArtifact={vi.fn()}/>);
 fireEvent.change(screen.getByLabelText('Search workspace entities'),{target:{value:a}});await waitFor(()=>expect(searchAll).toHaveBeenCalledWith(a));fireEvent.change(screen.getByLabelText('Search workspace entities'),{target:{value:b}});await screen.findByRole('option',{name:'Open Executive Summary'});await act(async()=>finish([{id:'a',title:'Old result',kind:'document',project_id:null,excerpt:''}]));expect(screen.queryByRole('option',{name:'Open Old result'})).not.toBeInTheDocument();expect(screen.getByRole('option',{name:'Open Executive Summary'})).toBeInTheDocument();
});
