import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Card, Group, Loader, Select, Stack, Text, Textarea, TextInput, Title } from '@mantine/core';
import { fetchReview, Review, saveReview } from '../shared/api/retrospective';

export function ExecutionReview({ id }: { id: string }) {
  const client = useQueryClient();
  const query = useQuery({queryKey:['review',id],queryFn:()=>fetchReview(id)});
  const [draft,setDraft] = useState<Review | null>(null);
  const value = draft ?? query.data ?? {outcome:'UNREVIEWED',notes:'',experiment:''};
  const mutation = useMutation({mutationFn:(review:Review)=>saveReview(id,review),onSuccess:async()=> {
    await Promise.all([client.invalidateQueries({queryKey:['review',id]}),client.invalidateQueries({queryKey:['retrospective']})]);
    setDraft(null);
  }});
  const change = (patch:Partial<Review>) => {setDraft({...value,...patch});mutation.reset();};
  return <Card withBorder radius="md" p="sm" mb="sm">
    <Title order={3}>Outcome review</Title><Text size="sm" c="dimmed" mb="md">Rate task quality independently of the process exit code.</Text>
    {query.isLoading ? <Loader size="sm"/> : query.isError ? <Alert color="red">{query.error.message} <Button size="xs" onClick={()=>query.refetch()}>Retry</Button></Alert> : <Stack>
      <Group grow align="flex-start"><Select label="Human outcome" allowDeselect={false} value={value.outcome} data={[{value:'UNREVIEWED',label:'Not reviewed'},{value:'ACCEPTED',label:'Accepted — useful as delivered'},{value:'REWORK',label:'Required rework'},{value:'REJECTED',label:'Rejected'}]} onChange={v=>change({outcome:v as Review['outcome']})} disabled={mutation.isPending}/>
        <TextInput label="Experiment label" description="Use the same label for comparable runs" placeholder="e.g. tests-first-v2" value={value.experiment} maxLength={120} onChange={e=>change({experiment:e.currentTarget.value})} disabled={mutation.isPending}/></Group>
      <Textarea label="Retrospective notes" placeholder="What worked? What needed correction? What will you change next time?" minRows={2} autosize maxLength={10000} value={value.notes} onChange={e=>change({notes:e.currentTarget.value})} disabled={mutation.isPending}/>
      <Group><Button onClick={()=>mutation.mutate(value)} loading={mutation.isPending} disabled={draft === null}>Save review</Button>
        {mutation.isSuccess && <Text size="sm" c="teal" role="status">Review saved locally.</Text>}
        {mutation.isError && <Text size="sm" c="red" role="alert">{mutation.error.message}</Text>}</Group>
    </Stack>}
  </Card>;
}
