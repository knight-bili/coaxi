import type {WorkspaceInspection} from '@contracts'

/** Recheck live UI and owner state after every await, before changing the destination. */
export async function prepareCreatedProject(input:{path:string;assertCurrent():void;inspect(path:string):Promise<WorkspaceInspection>;register(workspace:WorkspaceInspection):Promise<void>;enter(workspace:WorkspaceInspection):void}):Promise<void>{
  input.assertCurrent()
  const workspace=await input.inspect(input.path)
  input.assertCurrent()
  await input.register(workspace)
  input.assertCurrent()
  input.enter(workspace)
}
